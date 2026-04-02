#import <Foundation/Foundation.h>
#import <Speech/Speech.h>
#import <AVFoundation/AVFoundation.h>
#import <AVFAudio/AVFAudio.h>

typedef NS_ENUM(NSInteger, SuzakuSpeechState) {
    SuzakuSpeechStateUnavailable = 0,
    SuzakuSpeechStateReady = 1,
    SuzakuSpeechStateListening = 2,
    SuzakuSpeechStatePermissionPending = 3,
    SuzakuSpeechStateDenied = 4,
    SuzakuSpeechStateError = 5,
};

@interface SuzakuSpeechBridge : NSObject
@property (nonatomic, strong) SFSpeechRecognizer *recognizer;
@property (nonatomic, strong) AVAudioEngine *audioEngine;
@property (nonatomic, strong) SFSpeechAudioBufferRecognitionRequest *request;
@property (nonatomic, strong) SFSpeechRecognitionTask *task;
@property (nonatomic, copy) NSString *latestTranscript;
@property (nonatomic) SFSpeechRecognizerAuthorizationStatus speechAuth;
@property (nonatomic) BOOL micGranted;
@property (nonatomic) BOOL micResolved;
@property (nonatomic) SuzakuSpeechState state;
@end

@implementation SuzakuSpeechBridge

+ (instancetype)shared {
    static SuzakuSpeechBridge *bridge = nil;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        bridge = [[SuzakuSpeechBridge alloc] init];
    });
    return bridge;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _recognizer = [[SFSpeechRecognizer alloc] init];
        _audioEngine = [[AVAudioEngine alloc] init];
        _latestTranscript = @"";
        _speechAuth = [SFSpeechRecognizer authorizationStatus];
        _micGranted = NO;
        _micResolved = NO;
        _state = _recognizer != nil ? SuzakuSpeechStatePermissionPending : SuzakuSpeechStateUnavailable;
    }
    return self;
}

- (void)requestPermissionsIfNeeded {
    if (self.speechAuth == SFSpeechRecognizerAuthorizationStatusNotDetermined) {
        [SFSpeechRecognizer requestAuthorization:^(SFSpeechRecognizerAuthorizationStatus status) {
            @synchronized (self) {
                self.speechAuth = status;
                [self refreshState];
            }
        }];
    }

    if (@available(macOS 14.0, *)) {
        AVAudioApplicationRecordPermission permission = AVAudioApplication.sharedInstance.recordPermission;
        if (permission == AVAudioApplicationRecordPermissionGranted) {
            self.micGranted = YES;
            self.micResolved = YES;
        } else if (permission == AVAudioApplicationRecordPermissionDenied) {
            self.micGranted = NO;
            self.micResolved = YES;
        } else {
            [AVAudioApplication requestRecordPermissionWithCompletionHandler:^(BOOL granted) {
                @synchronized (self) {
                    self.micGranted = granted;
                    self.micResolved = YES;
                    [self refreshState];
                }
            }];
        }
    } else {
        self.micGranted = YES;
        self.micResolved = YES;
    }
}

- (void)refreshState {
    if (self.recognizer == nil) {
        self.state = SuzakuSpeechStateUnavailable;
        return;
    }
    if (self.speechAuth == SFSpeechRecognizerAuthorizationStatusDenied
        || self.speechAuth == SFSpeechRecognizerAuthorizationStatusRestricted
        || (self.micResolved && !self.micGranted)) {
        self.state = SuzakuSpeechStateDenied;
        return;
    }
    if (self.speechAuth != SFSpeechRecognizerAuthorizationStatusAuthorized || !self.micResolved) {
        self.state = SuzakuSpeechStatePermissionPending;
        return;
    }
    if (self.state != SuzakuSpeechStateListening) {
        self.state = SuzakuSpeechStateReady;
    }
}

- (BOOL)startListening {
    @synchronized (self) {
        [self requestPermissionsIfNeeded];
        [self refreshState];
        if (self.state != SuzakuSpeechStateReady) {
            return NO;
        }

        [self stopListening];
        self.latestTranscript = @"";
        self.request = [[SFSpeechAudioBufferRecognitionRequest alloc] init];
        self.request.shouldReportPartialResults = YES;
        if (@available(macOS 13.0, *)) {
            self.request.addsPunctuation = YES;
        }
        if (@available(macOS 10.15, *)) {
            self.request.requiresOnDeviceRecognition = NO;
        }

        AVAudioInputNode *inputNode = self.audioEngine.inputNode;
        AVAudioFormat *format = [inputNode outputFormatForBus:0];
        [inputNode removeTapOnBus:0];
        __weak typeof(self) weakSelf = self;
        [inputNode installTapOnBus:0
                        bufferSize:1024
                            format:format
                             block:^(AVAudioPCMBuffer *buffer, AVAudioTime *when) {
            (void)when;
            __strong typeof(weakSelf) strongSelf = weakSelf;
            if (strongSelf.request != nil) {
                [strongSelf.request appendAudioPCMBuffer:buffer];
            }
        }];

        NSError *error = nil;
        [self.audioEngine prepare];
        if (![self.audioEngine startAndReturnError:&error]) {
            self.state = SuzakuSpeechStateError;
            [inputNode removeTapOnBus:0];
            self.request = nil;
            return NO;
        }

        self.task = [self.recognizer recognitionTaskWithRequest:self.request
                                                  resultHandler:^(SFSpeechRecognitionResult *result, NSError *error) {
            __strong typeof(weakSelf) strongSelf = weakSelf;
            if (strongSelf == nil) {
                return;
            }
            @synchronized (strongSelf) {
                if (result != nil) {
                    strongSelf.latestTranscript = result.bestTranscription.formattedString ?: @"";
                }
                if (error != nil) {
                    strongSelf.state = SuzakuSpeechStateError;
                    [strongSelf stopListening];
                    return;
                }
                if (result != nil && result.isFinal) {
                    [strongSelf stopListening];
                    [strongSelf refreshState];
                }
            }
        }];

        self.state = SuzakuSpeechStateListening;
        return YES;
    }
}

- (void)stopListening {
    @synchronized (self) {
        [self.audioEngine.inputNode removeTapOnBus:0];
        if (self.audioEngine.isRunning) {
            [self.audioEngine stop];
        }
        [self.request endAudio];
        [self.task cancel];
        self.request = nil;
        self.task = nil;
        [self refreshState];
    }
}

- (BOOL)consumeTranscriptInto:(char *)buffer capacity:(size_t)capacity {
    @synchronized (self) {
        if (capacity == 0 || self.latestTranscript.length == 0) {
            return NO;
        }
        NSData *data = [self.latestTranscript dataUsingEncoding:NSUTF8StringEncoding];
        if (data.length + 1 > capacity) {
            return NO;
        }
        memcpy(buffer, data.bytes, data.length);
        buffer[data.length] = '\0';
        return YES;
    }
}

@end

bool suzaku_speech_is_supported(void) {
    return [SuzakuSpeechBridge shared].recognizer != nil;
}

int suzaku_speech_state(void) {
    return (int)[SuzakuSpeechBridge shared].state;
}

bool suzaku_speech_start(void) {
    return [[SuzakuSpeechBridge shared] startListening];
}

void suzaku_speech_stop(void) {
    [[SuzakuSpeechBridge shared] stopListening];
}

bool suzaku_speech_consume_transcript(char *buffer, size_t capacity) {
    return [[SuzakuSpeechBridge shared] consumeTranscriptInto:buffer capacity:capacity];
}

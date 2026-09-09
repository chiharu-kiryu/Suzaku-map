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

// Keep transcripts in memory only. Report failures through SuzakuSpeechState;
// neither recognition results nor framework error descriptions belong in logs.

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

- (BOOL)isRunningBundledApp {
    @try {
        NSString *bundlePath = [[NSBundle mainBundle] bundlePath];
        NSString *packageType = [[NSBundle mainBundle] objectForInfoDictionaryKey:@"CFBundlePackageType"];
        return bundlePath != nil
            && [bundlePath.pathExtension.lowercaseString isEqualToString:@"app"]
            && [packageType isEqualToString:@"APPL"];
    } @catch (NSException *exception) {
        (void)exception;
        return NO;
    }
}

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
    @try {
        self.speechAuth = [SFSpeechRecognizer authorizationStatus];
        BOOL bundled = [self isRunningBundledApp];
        if (bundled && self.speechAuth == SFSpeechRecognizerAuthorizationStatusNotDetermined) {
            [SFSpeechRecognizer requestAuthorization:^(SFSpeechRecognizerAuthorizationStatus status) {
                @synchronized (self) {
                    self.speechAuth = status;
                    [self refreshState];
                }
            }];
        }

        AVAuthorizationStatus micStatus =
            [AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio];
        if (micStatus == AVAuthorizationStatusAuthorized) {
            self.micGranted = YES;
            self.micResolved = YES;
        } else if (micStatus == AVAuthorizationStatusDenied
                   || micStatus == AVAuthorizationStatusRestricted) {
            self.micGranted = NO;
            self.micResolved = YES;
        } else if (micStatus == AVAuthorizationStatusNotDetermined) {
            [AVCaptureDevice requestAccessForMediaType:AVMediaTypeAudio
                                     completionHandler:^(BOOL granted) {
                @synchronized (self) {
                    self.micGranted = granted;
                    self.micResolved = YES;
                    [self refreshState];
                }
            }];
        } else {
            self.micGranted = NO;
            self.micResolved = NO;
        }
    } @catch (NSException *exception) {
        (void)exception;
        self.state = SuzakuSpeechStateError;
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

- (AVAudioFormat *)safeInputFormatForNode:(AVAudioInputNode *)inputNode {
    if (inputNode == nil) {
        return nil;
    }

    AVAudioFormat *format = nil;
    @try {
        format = [inputNode inputFormatForBus:0];
    } @catch (NSException *exception) {
        (void)exception;
        format = nil;
    }

    if (format == nil) {
        return nil;
    }
    if (format.sampleRate <= 0 || format.channelCount == 0) {
        return nil;
    }
    return format;
}

- (BOOL)startListening {
    @synchronized (self) {
        @try {
            [self requestPermissionsIfNeeded];
            [self refreshState];
            if (self.state != SuzakuSpeechStateReady || self.recognizer == nil) {
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
            if (inputNode == nil) {
                self.state = SuzakuSpeechStateError;
                self.request = nil;
                return NO;
            }

            AVAudioFormat *format = [self safeInputFormatForNode:inputNode];
            if (format == nil) {
                self.state = SuzakuSpeechStateError;
                self.request = nil;
                return NO;
            }

            [self.audioEngine stop];
            [self.audioEngine reset];
            [inputNode removeTapOnBus:0];
            __weak typeof(self) weakSelf = self;
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
                self.task = nil;
                return NO;
            }

            self.state = SuzakuSpeechStateListening;
            return YES;
        } @catch (NSException *exception) {
            (void)exception;
            self.state = SuzakuSpeechStateError;
            self.request = nil;
            self.task = nil;
            return NO;
        }
    }
}

- (void)stopListening {
    @synchronized (self) {
        @try {
            [self.audioEngine.inputNode removeTapOnBus:0];
            if (self.audioEngine.isRunning) {
                [self.audioEngine stop];
            }
            [self.request endAudio];
            [self.task cancel];
            self.request = nil;
            self.task = nil;
            [self refreshState];
        } @catch (NSException *exception) {
            (void)exception;
            self.request = nil;
            self.task = nil;
            self.state = SuzakuSpeechStateError;
        }
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

bool suzaku_speech_is_bundled_host(void) {
    return [[SuzakuSpeechBridge shared] isRunningBundledApp];
}

int suzaku_speech_state(void) {
    return (int)[SuzakuSpeechBridge shared].state;
}

void suzaku_speech_request_permissions(void) {
    SuzakuSpeechBridge *bridge = [SuzakuSpeechBridge shared];
    [bridge requestPermissionsIfNeeded];
    [bridge refreshState];
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

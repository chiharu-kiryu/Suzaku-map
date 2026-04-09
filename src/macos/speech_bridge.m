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

static void suzaku_voice_log(NSString *message) {
    @try {
        NSString *line = [NSString stringWithFormat:@"%@\n", message ?: @"(null)"];
        NSData *data = [line dataUsingEncoding:NSUTF8StringEncoding];
        if (data == nil) {
            return;
        }
        NSString *path = @"/tmp/suzaku-macos-voice.log";
        NSFileManager *manager = [NSFileManager defaultManager];
        if (![manager fileExistsAtPath:path]) {
            [data writeToFile:path atomically:YES];
            return;
        }
        NSFileHandle *handle = [NSFileHandle fileHandleForWritingAtPath:path];
        [handle seekToEndOfFile];
        [handle writeData:data];
        [handle closeFile];
    } @catch (NSException *exception) {
        (void)exception;
    }
}

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
        suzaku_voice_log([NSString stringWithFormat:@"init recognizer=%@ state=%ld",
                          _recognizer != nil ? @"yes" : @"no",
                          (long)_state]);
    }
    return self;
}

- (void)requestPermissionsIfNeeded {
    @try {
        suzaku_voice_log(@"requestPermissionsIfNeeded begin");
        self.speechAuth = [SFSpeechRecognizer authorizationStatus];
        suzaku_voice_log([NSString stringWithFormat:@"speech auth status=%ld", (long)self.speechAuth]);

        AVAuthorizationStatus micStatus =
            [AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio];
        suzaku_voice_log([NSString stringWithFormat:@"mic status=%ld", (long)micStatus]);
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
                    suzaku_voice_log([NSString stringWithFormat:@"mic callback granted=%@", granted ? @"yes" : @"no"]);
                    [self refreshState];
                }
            }];
        } else {
            self.micGranted = NO;
            self.micResolved = NO;
        }
    } @catch (NSException *exception) {
        suzaku_voice_log([NSString stringWithFormat:@"requestPermissions exception=%@", exception.reason ?: @"unknown"]);
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
    suzaku_voice_log([NSString stringWithFormat:@"refreshState -> %ld", (long)self.state]);
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
            suzaku_voice_log(@"startListening begin");
            [self requestPermissionsIfNeeded];
            [self refreshState];
            if (self.state != SuzakuSpeechStateReady || self.recognizer == nil) {
                suzaku_voice_log([NSString stringWithFormat:@"startListening abort early state=%ld recognizer=%@",
                                  (long)self.state,
                                  self.recognizer != nil ? @"yes" : @"no"]);
                return NO;
            }

            [self stopListening];
            suzaku_voice_log(@"startListening after stopListening");
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
                suzaku_voice_log(@"startListening inputNode=nil");
                self.state = SuzakuSpeechStateError;
                self.request = nil;
                return NO;
            }

            AVAudioFormat *format = [self safeInputFormatForNode:inputNode];
            if (format == nil) {
                suzaku_voice_log(@"startListening format=nil");
                self.state = SuzakuSpeechStateError;
                self.request = nil;
                return NO;
            }
            suzaku_voice_log([NSString stringWithFormat:@"startListening format sr=%.2f channels=%u",
                              format.sampleRate,
                              format.channelCount]);

            [self.audioEngine stop];
            [self.audioEngine reset];
            [inputNode removeTapOnBus:0];
            __weak typeof(self) weakSelf = self;
            suzaku_voice_log(@"startListening before recognitionTask");
            self.task = [self.recognizer recognitionTaskWithRequest:self.request
                                                      resultHandler:^(SFSpeechRecognitionResult *result, NSError *error) {
                __strong typeof(weakSelf) strongSelf = weakSelf;
                if (strongSelf == nil) {
                    return;
                }
                @synchronized (strongSelf) {
                    if (result != nil) {
                        strongSelf.latestTranscript = result.bestTranscription.formattedString ?: @"";
                        suzaku_voice_log([NSString stringWithFormat:@"recognition partial=%@ final=%@",
                                          strongSelf.latestTranscript,
                                          result.isFinal ? @"yes" : @"no"]);
                    }
                    if (error != nil) {
                        suzaku_voice_log([NSString stringWithFormat:@"recognition error=%@",
                                          error.localizedDescription ?: @"unknown"]);
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
            suzaku_voice_log(@"startListening before installTap");
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
            suzaku_voice_log(@"startListening before engine prepare");
            [self.audioEngine prepare];
            suzaku_voice_log(@"startListening before engine start");
            if (![self.audioEngine startAndReturnError:&error]) {
                suzaku_voice_log([NSString stringWithFormat:@"engine start failed=%@",
                                  error.localizedDescription ?: @"unknown"]);
                self.state = SuzakuSpeechStateError;
                [inputNode removeTapOnBus:0];
                self.request = nil;
                self.task = nil;
                return NO;
            }

            self.state = SuzakuSpeechStateListening;
            suzaku_voice_log(@"startListening success");
            return YES;
        } @catch (NSException *exception) {
            suzaku_voice_log([NSString stringWithFormat:@"startListening exception=%@", exception.reason ?: @"unknown"]);
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
            suzaku_voice_log(@"stopListening begin");
            [self.audioEngine.inputNode removeTapOnBus:0];
            if (self.audioEngine.isRunning) {
                [self.audioEngine stop];
            }
            [self.request endAudio];
            [self.task cancel];
            self.request = nil;
            self.task = nil;
            [self refreshState];
            suzaku_voice_log(@"stopListening end");
        } @catch (NSException *exception) {
            suzaku_voice_log([NSString stringWithFormat:@"stopListening exception=%@", exception.reason ?: @"unknown"]);
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

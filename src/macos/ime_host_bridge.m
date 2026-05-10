#import <Foundation/Foundation.h>

#if __has_include(<InputMethodKit/InputMethodKit.h>)
#import <InputMethodKit/InputMethodKit.h>
#endif

#if __has_include(<InputMethodKit/InputMethodKit.h>)
@interface SuzakuInputController : IMKInputController
@end

bool suzaku_host_ime_activate(void);
void suzaku_host_ime_deactivate(void);
bool suzaku_host_ime_replace_marked_text_utf8(const char *rawText);
void suzaku_host_ime_clear_marked_text(void);
void suzaku_host_ime_move_selection(long delta);
void suzaku_host_ime_select_candidate(unsigned long index);
bool suzaku_host_ime_commit_selected(bool force);
char *suzaku_host_ime_display_text_utf8(void);
char *suzaku_host_ime_take_last_committed_text_utf8(void);
void suzaku_host_ime_free_utf8(char *rawText);

static NSUInteger suzakuControllerInitCount = 0;
static NSUInteger suzakuControllerActivateCount = 0;
static NSUInteger suzakuControllerDeactivateCount = 0;
static NSUInteger suzakuControllerInputCount = 0;
static NSUInteger suzakuControllerCommitCount = 0;
static BOOL suzakuControllerActive = NO;
static NSString *suzakuControllerLastMarkedText = nil;

static NSString *suzakuBridgeTakeNSString(char *(*provider)(void)) {
    char *raw = provider();
    if (raw == NULL) {
        return nil;
    }
    NSString *value = [[NSString alloc] initWithUTF8String:raw];
    suzaku_host_ime_free_utf8(raw);
    return value;
}

static void suzakuApplyMarkedTextToClient(id client) {
    if (client == nil) {
        return;
    }

    NSString *displayText = suzakuBridgeTakeNSString(suzaku_host_ime_display_text_utf8);
    if (displayText == nil || displayText.length == 0) {
        if ([client respondsToSelector:@selector(unmarkText)]) {
            [client unmarkText];
        } else if ([client respondsToSelector:@selector(setMarkedText:selectionRange:replacementRange:)]) {
            [client setMarkedText:@"" selectionRange:NSMakeRange(0, 0) replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
        }
        return;
    }

    if ([client respondsToSelector:@selector(setMarkedText:selectionRange:replacementRange:)]) {
        [client setMarkedText:displayText
               selectionRange:NSMakeRange(displayText.length, 0)
             replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
    }
}

static void suzakuCommitTextToClient(id client) {
    if (client == nil) {
        return;
    }

    NSString *committedText = suzakuBridgeTakeNSString(suzaku_host_ime_take_last_committed_text_utf8);
    if (committedText == nil || committedText.length == 0) {
        return;
    }

    if ([client respondsToSelector:@selector(insertText:replacementRange:)]) {
        [client insertText:committedText replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
    } else if ([client respondsToSelector:@selector(insertText:)]) {
        [client insertText:committedText];
    }

    if ([client respondsToSelector:@selector(unmarkText)]) {
        [client unmarkText];
    }
}

@implementation SuzakuInputController
- (instancetype)initWithServer:(IMKServer *)server delegate:(id)delegate client:(id)inputClient {
    self = [super initWithServer:server delegate:delegate client:inputClient];
    if (self != nil) {
        suzakuControllerInitCount += 1;
    }
    return self;
}

- (void)activateServer:(id)sender {
    suzakuControllerActive = YES;
    suzakuControllerActivateCount += 1;
    suzaku_host_ime_activate();
    [super activateServer:sender];
}

- (void)deactivateServer:(id)sender {
    suzakuControllerActive = NO;
    suzakuControllerDeactivateCount += 1;
    suzaku_host_ime_deactivate();
    [super deactivateServer:sender];
}

- (BOOL)inputText:(NSString *)string key:(NSInteger)keyCode modifiers:(NSUInteger)flags client:(id)sender {
    (void)keyCode;
    (void)flags;
    (void)sender;
    suzakuControllerInputCount += 1;
    suzakuControllerLastMarkedText = [string copy];
    const char *utf8 = [string UTF8String];
    if (utf8 == NULL) {
        return NO;
    }
    BOOL accepted = suzaku_host_ime_replace_marked_text_utf8(utf8);
    if (accepted) {
        suzakuApplyMarkedTextToClient(sender);
    }
    return accepted;
}

- (void)doCommandBySelector:(SEL)selector client:(id)sender {
    if (selector == @selector(moveUp:)) {
        suzaku_host_ime_move_selection(-1);
        suzakuApplyMarkedTextToClient(sender);
        return;
    }
    if (selector == @selector(moveDown:)) {
        suzaku_host_ime_move_selection(1);
        suzakuApplyMarkedTextToClient(sender);
        return;
    }
    if (selector == @selector(cancelOperation:)) {
        suzaku_host_ime_clear_marked_text();
        suzakuControllerLastMarkedText = nil;
        suzakuApplyMarkedTextToClient(sender);
        return;
    }
    if (selector == @selector(insertNewline:)) {
        if (suzaku_host_ime_commit_selected(true)) {
            suzakuControllerLastMarkedText = nil;
            suzakuCommitTextToClient(sender);
            return;
        }
    }
    (void)sender;
}

- (void)commitComposition:(id)sender {
    suzakuControllerCommitCount += 1;
    suzakuControllerLastMarkedText = nil;
    if (suzaku_host_ime_commit_selected(true)) {
        suzakuCommitTextToClient(sender);
    }
    [super commitComposition:sender];
}
@end

static IMKServer *suzakuIMKServer = nil;
#endif

bool suzaku_input_methodkit_available(void) {
    @autoreleasepool {
#if __has_include(<InputMethodKit/InputMethodKit.h>)
        return NSClassFromString(@"IMKServer") != nil;
#else
        return false;
#endif
    }
}

bool suzaku_input_methodkit_bundled_runtime(void) {
    @autoreleasepool {
        NSString *bundlePath = [[NSBundle mainBundle] bundlePath];
        NSString *packageType = [[NSBundle mainBundle] objectForInfoDictionaryKey:@"CFBundlePackageType"];
        return bundlePath != nil
            && [bundlePath.pathExtension.lowercaseString isEqualToString:@"app"]
            && [packageType isEqualToString:@"APPL"];
    }
}

char *suzaku_input_methodkit_main_bundle_identifier(void) {
    @autoreleasepool {
        NSString *bundleIdentifier = [[NSBundle mainBundle] bundleIdentifier];
        if (bundleIdentifier == nil) {
            return NULL;
        }
        const char *utf8 = [bundleIdentifier UTF8String];
        if (utf8 == NULL) {
            return NULL;
        }
        return strdup(utf8);
    }
}

void suzaku_input_methodkit_free_c_string(char *value) {
    if (value != NULL) {
        free(value);
    }
}

char *suzaku_input_methodkit_connection_name(void) {
    @autoreleasepool {
        NSString *connectionName = [[NSBundle mainBundle] objectForInfoDictionaryKey:@"InputMethodConnectionName"];
        if (connectionName == nil || connectionName.length == 0) {
            return NULL;
        }
        const char *utf8 = [connectionName UTF8String];
        if (utf8 == NULL) {
            return NULL;
        }
        return strdup(utf8);
    }
}

bool suzaku_input_methodkit_bootstrap_server(void) {
    @autoreleasepool {
#if __has_include(<InputMethodKit/InputMethodKit.h>)
        @try {
            NSString *bundleIdentifier = [[NSBundle mainBundle] bundleIdentifier];
            NSString *connectionName = [[NSBundle mainBundle] objectForInfoDictionaryKey:@"InputMethodConnectionName"];
            if (bundleIdentifier == nil || connectionName == nil || connectionName.length == 0) {
                return false;
            }
            if (suzakuIMKServer != nil) {
                return true;
            }
            suzakuIMKServer =
                [[IMKServer alloc] initWithName:connectionName bundleIdentifier:bundleIdentifier];
            return suzakuIMKServer != nil;
        } @catch (NSException *exception) {
            (void)exception;
            return false;
        }
#else
        return false;
#endif
    }
}

char *suzaku_input_methodkit_controller_class_name(void) {
    @autoreleasepool {
#if __has_include(<InputMethodKit/InputMethodKit.h>)
        const char *utf8 = "SuzakuInputController";
        return strdup(utf8);
#else
        return NULL;
#endif
    }
}

bool suzaku_input_methodkit_controller_lifecycle_ready(void) {
    @autoreleasepool {
#if __has_include(<InputMethodKit/InputMethodKit.h>)
        return NSClassFromString(@"SuzakuInputController") != nil;
#else
        return false;
#endif
    }
}

void suzaku_input_methodkit_reset_controller_debug_state(void) {
#if __has_include(<InputMethodKit/InputMethodKit.h>)
    @autoreleasepool {
        suzakuControllerInitCount = 0;
        suzakuControllerActivateCount = 0;
        suzakuControllerDeactivateCount = 0;
        suzakuControllerInputCount = 0;
        suzakuControllerCommitCount = 0;
        suzakuControllerActive = NO;
        suzakuControllerLastMarkedText = nil;
    }
#endif
}

unsigned long suzaku_input_methodkit_controller_init_count(void) {
#if __has_include(<InputMethodKit/InputMethodKit.h>)
    return (unsigned long)suzakuControllerInitCount;
#else
    return 0;
#endif
}

unsigned long suzaku_input_methodkit_controller_activate_count(void) {
#if __has_include(<InputMethodKit/InputMethodKit.h>)
    return (unsigned long)suzakuControllerActivateCount;
#else
    return 0;
#endif
}

unsigned long suzaku_input_methodkit_controller_deactivate_count(void) {
#if __has_include(<InputMethodKit/InputMethodKit.h>)
    return (unsigned long)suzakuControllerDeactivateCount;
#else
    return 0;
#endif
}

unsigned long suzaku_input_methodkit_controller_input_count(void) {
#if __has_include(<InputMethodKit/InputMethodKit.h>)
    return (unsigned long)suzakuControllerInputCount;
#else
    return 0;
#endif
}

unsigned long suzaku_input_methodkit_controller_commit_count(void) {
#if __has_include(<InputMethodKit/InputMethodKit.h>)
    return (unsigned long)suzakuControllerCommitCount;
#else
    return 0;
#endif
}

bool suzaku_input_methodkit_controller_is_active(void) {
#if __has_include(<InputMethodKit/InputMethodKit.h>)
    return suzakuControllerActive;
#else
    return false;
#endif
}

char *suzaku_input_methodkit_controller_last_marked_text(void) {
    @autoreleasepool {
#if __has_include(<InputMethodKit/InputMethodKit.h>)
        if (suzakuControllerLastMarkedText == nil || suzakuControllerLastMarkedText.length == 0) {
            return NULL;
        }
        const char *utf8 = [suzakuControllerLastMarkedText UTF8String];
        if (utf8 == NULL) {
            return NULL;
        }
        return strdup(utf8);
#else
        return NULL;
#endif
    }
}

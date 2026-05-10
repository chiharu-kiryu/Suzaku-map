#import <Foundation/Foundation.h>
#import <AppKit/AppKit.h>
#import <ApplicationServices/ApplicationServices.h>

typedef NS_ENUM(NSInteger, SuzakuTextOutputPermissionState) {
    SuzakuTextOutputUnavailable = 0,
    SuzakuTextOutputReady = 1,
    SuzakuTextOutputPermissionRequired = 2,
    SuzakuTextOutputError = 3,
};

static BOOL suzaku_ax_trusted(BOOL prompt) {
    CFBooleanRef promptValue = prompt ? kCFBooleanTrue : kCFBooleanFalse;
    const void *keys[] = { kAXTrustedCheckOptionPrompt };
    const void *values[] = { promptValue };
    CFDictionaryRef options = CFDictionaryCreate(
        kCFAllocatorDefault,
        keys,
        values,
        1,
        &kCFCopyStringDictionaryKeyCallBacks,
        &kCFTypeDictionaryValueCallBacks
    );
    BOOL trusted = AXIsProcessTrustedWithOptions(options);
    CFRelease(options);
    return trusted;
}

int suzaku_text_output_permission_state(bool prompt) {
    @autoreleasepool {
        @try {
            return suzaku_ax_trusted(prompt) ? SuzakuTextOutputReady
                                             : SuzakuTextOutputPermissionRequired;
        } @catch (NSException *exception) {
            (void)exception;
            return SuzakuTextOutputError;
        }
    }
}

bool suzaku_text_output_commit_utf8(const char *text) {
    @autoreleasepool {
        @try {
            if (text == NULL || text[0] == '\0') {
                return false;
            }
            if (!suzaku_ax_trusted(NO)) {
                return false;
            }

            NSString *string = [NSString stringWithUTF8String:text];
            if (string == nil || string.length == 0) {
                return false;
            }

            NSPasteboard *pasteboard = [NSPasteboard generalPasteboard];
            NSString *previousString = [[pasteboard stringForType:NSPasteboardTypeString] copy];
            NSInteger previousChangeCount = pasteboard.changeCount;
            [pasteboard clearContents];
            if (![pasteboard setString:string forType:NSPasteboardTypeString]) {
                return false;
            }

            CGEventSourceRef source = CGEventSourceCreate(kCGEventSourceStateHIDSystemState);
            if (source == NULL) {
                return false;
            }

            CGEventRef keyDown = CGEventCreateKeyboardEvent(source, (CGKeyCode)0x09, true);
            CGEventRef keyUp = CGEventCreateKeyboardEvent(source, (CGKeyCode)0x09, false);
            if (keyDown == NULL || keyUp == NULL) {
                if (keyDown != NULL) {
                    CFRelease(keyDown);
                }
                if (keyUp != NULL) {
                    CFRelease(keyUp);
                }
                CFRelease(source);
                return false;
            }

            CGEventSetFlags(keyDown, kCGEventFlagMaskCommand);
            CGEventSetFlags(keyUp, kCGEventFlagMaskCommand);
            CGEventPost(kCGAnnotatedSessionEventTap, keyDown);
            CGEventPost(kCGAnnotatedSessionEventTap, keyUp);

            CFRelease(keyDown);
            CFRelease(keyUp);
            CFRelease(source);

            dispatch_after(
                dispatch_time(DISPATCH_TIME_NOW, (int64_t)(250 * NSEC_PER_MSEC)),
                dispatch_get_main_queue(),
                ^{
                    @autoreleasepool {
                        NSPasteboard *restorePasteboard = [NSPasteboard generalPasteboard];
                        if (restorePasteboard.changeCount != previousChangeCount + 1) {
                            return;
                        }
                        [restorePasteboard clearContents];
                        if (previousString.length > 0) {
                            [restorePasteboard setString:previousString
                                                 forType:NSPasteboardTypeString];
                        }
                    }
                }
            );

            return true;
        } @catch (NSException *exception) {
            (void)exception;
            return false;
        }
    }
}


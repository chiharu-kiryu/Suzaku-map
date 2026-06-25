bool suzaku_input_methodkit_available(void) {
    @autoreleasepool {
#if __has_include(<InputMethodKit/InputMethodKit.h>)
        return NSClassFromString(@"IMKServer") != nil;
#else
        return false;
#endif
    }
}

char *suzaku_input_methodkit_adapter_summary(void) {
    @autoreleasepool {
        NSString *summary =
            suzakuBridgeTakePlatformNSString(suzaku_host_platform_adapter_summary_utf8);
        if (summary == nil || summary.length == 0) {
            return NULL;
        }
        const char *utf8 = [summary UTF8String];
        if (utf8 == NULL) {
            return NULL;
        }
        return strdup(utf8);
    }
}

char *suzaku_input_methodkit_registration_target(void) {
    @autoreleasepool {
        NSString *target =
            suzakuBridgeTakePlatformNSString(suzaku_host_platform_registration_target_utf8);
        if (target == nil || target.length == 0) {
            return NULL;
        }
        const char *utf8 = [target UTF8String];
        if (utf8 == NULL) {
            return NULL;
        }
        return strdup(utf8);
    }
}

char *suzaku_input_methodkit_registration_hint(void) {
    @autoreleasepool {
        NSString *hint =
            suzakuBridgeTakePlatformNSString(suzaku_host_platform_registration_hint_utf8);
        if (hint == nil || hint.length == 0) {
            return NULL;
        }
        const char *utf8 = [hint UTF8String];
        if (utf8 == NULL) {
            return NULL;
        }
        return strdup(utf8);
    }
}

bool suzaku_input_methodkit_registration_ready(void) {
    return suzaku_host_platform_registration_ready();
}

bool suzaku_input_methodkit_on_demand_companion(void) {
    return suzaku_host_platform_on_demand_companion();
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
        suzakuCandidateCompanionRefreshCount = 0;
        suzakuCandidateCompanionVisible = NO;
        suzakuCandidateCompanionSelectedIndex = 0;
        suzakuCandidateCompanionHoveredIndex = NSNotFound;
        suzakuCandidateCompanionPrimaryCandidate = nil;
        suzakuHideCandidateCompanionPanel();
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

unsigned long suzaku_input_methodkit_candidate_companion_refresh_count(void) {
#if __has_include(<InputMethodKit/InputMethodKit.h>)
    return (unsigned long)suzakuCandidateCompanionRefreshCount;
#else
    return 0;
#endif
}

bool suzaku_input_methodkit_candidate_companion_visible(void) {
#if __has_include(<InputMethodKit/InputMethodKit.h>)
    return suzakuCandidateCompanionVisible
        && suzakuCandidateCompanionPanel != nil
        && [suzakuCandidateCompanionPanel isVisible];
#else
    return false;
#endif
}

unsigned long suzaku_input_methodkit_candidate_companion_selected_index(void) {
#if __has_include(<InputMethodKit/InputMethodKit.h>)
    return (unsigned long)suzakuCandidateCompanionSelectedIndex;
#else
    return 0;
#endif
}

unsigned long suzaku_input_methodkit_candidate_companion_hovered_index(void) {
#if __has_include(<InputMethodKit/InputMethodKit.h>)
    return suzakuCandidateCompanionHoveredIndex == NSNotFound
        ? ULONG_MAX
        : (unsigned long)suzakuCandidateCompanionHoveredIndex;
#else
    return ULONG_MAX;
#endif
}

char *suzaku_input_methodkit_candidate_companion_primary_candidate(void) {
    @autoreleasepool {
#if __has_include(<InputMethodKit/InputMethodKit.h>)
        if (suzakuCandidateCompanionPrimaryCandidate == nil
            || suzakuCandidateCompanionPrimaryCandidate.length == 0) {
            return NULL;
        }
        const char *utf8 = [suzakuCandidateCompanionPrimaryCandidate UTF8String];
        if (utf8 == NULL) {
            return NULL;
        }
        return strdup(utf8);
#else
        return NULL;
#endif
    }
}

bool suzaku_input_methodkit_candidate_companion_window_ready(void) {
    @autoreleasepool {
#if __has_include(<InputMethodKit/InputMethodKit.h>)
        return NSClassFromString(@"NSPanel") != nil
            && NSClassFromString(@"NSTextField") != nil;
#else
        return false;
#endif
    }
}

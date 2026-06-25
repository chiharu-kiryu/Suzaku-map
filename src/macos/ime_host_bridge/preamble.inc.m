@interface SuzakuCandidateButton : NSButton
@property(nonatomic, assign) NSUInteger suzakuCandidateIndex;
@end

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
unsigned long suzaku_host_ime_candidate_count(void);
unsigned long suzaku_host_ime_selected_index(void);
char *suzaku_host_ime_candidate_label_utf8(unsigned long index);
char *suzaku_host_ime_primary_candidate_utf8(void);
void suzaku_host_ime_free_utf8(char *rawText);
char *suzaku_host_platform_adapter_summary_utf8(void);
char *suzaku_host_platform_registration_target_utf8(void);
char *suzaku_host_platform_registration_hint_utf8(void);
bool suzaku_host_platform_registration_ready(void);
bool suzaku_host_platform_on_demand_companion(void);
void suzaku_host_platform_free_utf8(char *rawText);
char *suzaku_host_companion_window_title_utf8(void);
char *suzaku_host_companion_header_title_utf8(void);
bool suzaku_host_companion_show_header(void);
unsigned short suzaku_host_companion_min_width(void);
unsigned short suzaku_host_companion_max_width(void);
unsigned short suzaku_host_companion_row_height(void);
unsigned char suzaku_host_companion_max_text_lines(void);
unsigned short suzaku_host_companion_horizontal_padding(void);
unsigned short suzaku_host_companion_vertical_padding(void);
unsigned int suzaku_host_companion_panel_background_rgba8(void);
unsigned int suzaku_host_companion_title_text_rgba8(void);
unsigned int suzaku_host_companion_row_normal_text_rgba8(void);
unsigned int suzaku_host_companion_row_hover_text_rgba8(void);
unsigned int suzaku_host_companion_row_selected_text_rgba8(void);
unsigned int suzaku_host_companion_accent_rgba8(void);
unsigned int suzaku_host_companion_border_rgba8(void);

static NSUInteger suzakuControllerInitCount = 0;
static NSUInteger suzakuControllerActivateCount = 0;
static NSUInteger suzakuControllerDeactivateCount = 0;
static NSUInteger suzakuControllerInputCount = 0;
static NSUInteger suzakuControllerCommitCount = 0;
static BOOL suzakuControllerActive = NO;
static NSString *suzakuControllerLastMarkedText = nil;
static NSUInteger suzakuCandidateCompanionRefreshCount = 0;
static BOOL suzakuCandidateCompanionVisible = NO;
static NSUInteger suzakuCandidateCompanionSelectedIndex = 0;
static NSUInteger suzakuCandidateCompanionHoveredIndex = NSNotFound;
static NSString *suzakuCandidateCompanionPrimaryCandidate = nil;
static NSPanel *suzakuCandidateCompanionPanel = nil;
static NSMutableArray<NSButton *> *suzakuCandidateCompanionButtons = nil;
static NSTextField *suzakuCandidateCompanionHeader = nil;
static __weak id suzakuActiveClient = nil;
static __weak SuzakuInputController *suzakuActiveController = nil;

static void suzakuRefreshCandidateCompanion(void);

@implementation SuzakuCandidateButton {
    NSTrackingArea *_suzakuTrackingArea;
}

- (void)updateTrackingAreas {
    [super updateTrackingAreas];
    if (_suzakuTrackingArea != nil) {
        [self removeTrackingArea:_suzakuTrackingArea];
    }
    _suzakuTrackingArea = [[NSTrackingArea alloc]
        initWithRect:self.bounds
             options:NSTrackingActiveAlways | NSTrackingMouseEnteredAndExited | NSTrackingInVisibleRect
               owner:self
            userInfo:nil];
    [self addTrackingArea:_suzakuTrackingArea];
}

- (void)mouseEntered:(NSEvent *)event {
    (void)event;
    suzakuCandidateCompanionHoveredIndex = self.suzakuCandidateIndex;
    suzakuRefreshCandidateCompanion();
}

- (void)mouseExited:(NSEvent *)event {
    (void)event;
    if (suzakuCandidateCompanionHoveredIndex == self.suzakuCandidateIndex) {
        suzakuCandidateCompanionHoveredIndex = NSNotFound;
        suzakuRefreshCandidateCompanion();
    }
}
@end

static NSString *suzakuBridgeTakeNSString(char *(*provider)(void)) {
    char *raw = provider();
    if (raw == NULL) {
        return nil;
    }
    NSString *value = [[NSString alloc] initWithUTF8String:raw];
    suzaku_host_ime_free_utf8(raw);
    return value;
}

static NSString *suzakuBridgeTakeCandidateLabel(NSUInteger index) {
    char *raw = suzaku_host_ime_candidate_label_utf8((unsigned long)index);
    if (raw == NULL) {
        return nil;
    }
    NSString *value = [[NSString alloc] initWithUTF8String:raw];
    suzaku_host_ime_free_utf8(raw);
    return value;
}

static NSString *suzakuBridgeTakePlatformNSString(char *(*provider)(void)) {
    char *raw = provider();
    if (raw == NULL) {
        return nil;
    }
    NSString *value = [[NSString alloc] initWithUTF8String:raw];
    suzaku_host_platform_free_utf8(raw);
    return value;
}

static NSColor *suzakuColorFromPackedRGBA(unsigned int packedColor) {
    CGFloat red = ((packedColor >> 24) & 0xFF) / 255.0;
    CGFloat green = ((packedColor >> 16) & 0xFF) / 255.0;
    CGFloat blue = ((packedColor >> 8) & 0xFF) / 255.0;
    CGFloat alpha = (packedColor & 0xFF) / 255.0;
    return [NSColor colorWithCalibratedRed:red green:green blue:blue alpha:alpha];
}

static NSAttributedString *suzakuCandidateAttributedTitle(
    NSString *text,
    NSColor *color,
    NSFont *font
) {
    NSMutableParagraphStyle *paragraph = [[NSMutableParagraphStyle alloc] init];
    [paragraph setLineBreakMode:NSLineBreakByTruncatingTail];
    [paragraph setLineSpacing:1.0];
    [paragraph setAlignment:NSTextAlignmentLeft];
    NSDictionary *attributes = @{
        NSForegroundColorAttributeName : color,
        NSFontAttributeName : font,
        NSParagraphStyleAttributeName : paragraph,
    };
    return [[NSAttributedString alloc] initWithString:text attributes:attributes];
}

static void suzakuPositionCandidateCompanionNearClient(id client) {
    if (suzakuCandidateCompanionPanel == nil) {
        return;
    }

    NSRect anchorRect = NSZeroRect;
    @try {
        NSRange targetRange = NSMakeRange(0, 0);
        if (client != nil && [client respondsToSelector:@selector(selectedRange)]) {
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Warc-performSelector-leaks"
            NSValue *selectedRangeValue = [client performSelector:@selector(selectedRange)];
#pragma clang diagnostic pop
            if ([selectedRangeValue isKindOfClass:[NSValue class]]) {
                [selectedRangeValue getValue:&targetRange];
            }
        }
        if (client != nil
            && [client respondsToSelector:@selector(firstRectForCharacterRange:actualRange:)]) {
            NSRange actualRange = NSMakeRange(NSNotFound, 0);
            anchorRect = [client firstRectForCharacterRange:targetRange
                                                actualRange:&actualRange];
        }
    } @catch (NSException *exception) {
        (void)exception;
    }

    NSRect frame = [suzakuCandidateCompanionPanel frame];
    NSScreen *screen = [NSScreen mainScreen];
    NSRect visibleFrame = screen != nil ? [screen visibleFrame] : NSMakeRect(0, 0, 1280, 800);

    CGFloat x = visibleFrame.origin.x + 48.0;
    CGFloat y = visibleFrame.origin.y + visibleFrame.size.height - frame.size.height - 96.0;

    if (!NSEqualRects(anchorRect, NSZeroRect) && !NSIsEmptyRect(anchorRect)) {
        x = anchorRect.origin.x;
        y = anchorRect.origin.y - frame.size.height - 10.0;
    }

    x = MAX(visibleFrame.origin.x + 12.0,
            MIN(x, NSMaxX(visibleFrame) - frame.size.width - 12.0));
    y = MAX(visibleFrame.origin.y + 12.0,
            MIN(y, NSMaxY(visibleFrame) - frame.size.height - 12.0));

    [suzakuCandidateCompanionPanel setFrameOrigin:NSMakePoint(x, y)];
}

static void suzakuEnsureCandidateCompanionPanel(void) {
    if (suzakuCandidateCompanionPanel != nil) {
        return;
    }

    if ([NSApplication sharedApplication] == nil) {
        return;
    }

    NSRect frame = NSMakeRect(96.0, 96.0, 320.0, 220.0);
    suzakuCandidateCompanionPanel = [[NSPanel alloc]
        initWithContentRect:frame
                  styleMask:NSWindowStyleMaskNonactivatingPanel | NSWindowStyleMaskFullSizeContentView
                    backing:NSBackingStoreBuffered
                      defer:NO];
    [suzakuCandidateCompanionPanel setFloatingPanel:YES];
    [suzakuCandidateCompanionPanel setHidesOnDeactivate:NO];
    [suzakuCandidateCompanionPanel setBecomesKeyOnlyIfNeeded:NO];
    [suzakuCandidateCompanionPanel setReleasedWhenClosed:NO];
    [suzakuCandidateCompanionPanel setLevel:NSStatusWindowLevel];
    [suzakuCandidateCompanionPanel setOpaque:NO];
    NSString *windowTitle = suzakuBridgeTakeNSString(suzaku_host_companion_window_title_utf8);
    [suzakuCandidateCompanionPanel setBackgroundColor:suzakuColorFromPackedRGBA(
                                              suzaku_host_companion_panel_background_rgba8())];
    [suzakuCandidateCompanionPanel setTitleVisibility:NSWindowTitleHidden];
    [suzakuCandidateCompanionPanel setTitlebarAppearsTransparent:YES];
    [suzakuCandidateCompanionPanel setMovableByWindowBackground:YES];
    [suzakuCandidateCompanionPanel
        setTitle:windowTitle != nil ? windowTitle : @"Suzaku Candidates"];

    NSView *contentView = [suzakuCandidateCompanionPanel contentView];
    suzakuCandidateCompanionButtons = [NSMutableArray array];

    NSTextField *header = [[NSTextField alloc] initWithFrame:NSMakeRect(12.0, 184.0, 296.0, 18.0)];
    NSString *headerTitle = suzakuBridgeTakeNSString(suzaku_host_companion_header_title_utf8);
    [header setStringValue:headerTitle != nil ? headerTitle : @"Suzaku Candidates"];
    [header setBezeled:NO];
    [header setDrawsBackground:NO];
    [header setEditable:NO];
    [header setSelectable:NO];
    [header setTextColor:suzakuColorFromPackedRGBA(suzaku_host_companion_title_text_rgba8())];
    [header setFont:[NSFont systemFontOfSize:11.0 weight:NSFontWeightSemibold]];
    [contentView addSubview:header];
    suzakuCandidateCompanionHeader = header;

    for (NSUInteger index = 0; index < 6; index += 1) {
        SuzakuCandidateButton *button =
            [[SuzakuCandidateButton alloc] initWithFrame:NSMakeRect(12.0, 148.0 - (CGFloat)index * 22.0, 296.0, 20.0)];
        [button setBordered:NO];
        [button setButtonType:NSButtonTypeMomentaryChange];
        [button setBezelStyle:NSBezelStyleRegularSquare];
        [button setAlignment:NSTextAlignmentLeft];
        [button setHidden:YES];
        [button setFont:[NSFont systemFontOfSize:13.0 weight:NSFontWeightMedium]];
        [[button cell] setWraps:YES];
        [[button cell] setScrollable:NO];
        [[button cell] setLineBreakMode:NSLineBreakByTruncatingTail];
        [button setImagePosition:NSNoImage];
        [button setSuzakuCandidateIndex:index];
        [button setTarget:nil];
        [button setAction:NULL];
        [contentView addSubview:button];
        [suzakuCandidateCompanionButtons addObject:button];
    }
}

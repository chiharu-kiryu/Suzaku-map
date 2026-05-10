#import <Foundation/Foundation.h>
#import <limits.h>

#if __has_include(<InputMethodKit/InputMethodKit.h>)
#import <InputMethodKit/InputMethodKit.h>
#endif

#if __has_include(<InputMethodKit/InputMethodKit.h>)
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

static void suzakuHideCandidateCompanionPanel(void) {
    suzakuCandidateCompanionVisible = NO;
    suzakuCandidateCompanionHoveredIndex = NSNotFound;
    if (suzakuCandidateCompanionPanel != nil) {
        [suzakuCandidateCompanionPanel orderOut:nil];
    }
}

static void suzakuUpdateCandidateCompanionPanel(void) {
    @try {
        suzakuEnsureCandidateCompanionPanel();
    } @catch (NSException *exception) {
        (void)exception;
        suzakuCandidateCompanionVisible = NO;
        return;
    }

    if (suzakuCandidateCompanionPanel == nil) {
        suzakuCandidateCompanionVisible = NO;
        return;
    }

    NSUInteger candidateCount = (NSUInteger)suzaku_host_ime_candidate_count();
    suzakuCandidateCompanionSelectedIndex = (NSUInteger)suzaku_host_ime_selected_index();
    suzakuCandidateCompanionPrimaryCandidate =
        suzakuBridgeTakeNSString(suzaku_host_ime_primary_candidate_utf8);
    if (suzakuCandidateCompanionHoveredIndex >= candidateCount) {
        suzakuCandidateCompanionHoveredIndex = NSNotFound;
    }

    if (candidateCount == 0) {
        suzakuHideCandidateCompanionPanel();
        return;
    }

    BOOL showHeader = suzaku_host_companion_show_header();
    CGFloat horizontalPadding = (CGFloat)suzaku_host_companion_horizontal_padding();
    CGFloat verticalPadding = (CGFloat)suzaku_host_companion_vertical_padding();
    CGFloat rowHeight = (CGFloat)suzaku_host_companion_row_height();
    NSUInteger maxTextLines = MAX((NSUInteger)suzaku_host_companion_max_text_lines(), (NSUInteger)1);
    CGFloat minWidth = (CGFloat)suzaku_host_companion_min_width();
    CGFloat maxWidth = (CGFloat)suzaku_host_companion_max_width();
    NSDictionary *measureAttributes = @{
        NSFontAttributeName : [NSFont systemFontOfSize:13.0 weight:NSFontWeightMedium]
    };
    CGFloat measuredWidth = minWidth;
    NSUInteger measuredCount = MIN(candidateCount, (NSUInteger)[suzakuCandidateCompanionButtons count]);
    for (NSUInteger index = 0; index < measuredCount; index += 1) {
        NSString *text = suzakuBridgeTakeCandidateLabel(index);
        if (text == nil) {
            continue;
        }
        NSString *prefixed = [NSString stringWithFormat:@"› %@", text];
        CGFloat candidateWidth =
            ceil([prefixed sizeWithAttributes:measureAttributes].width) + horizontalPadding * 2.0;
        measuredWidth = MAX(measuredWidth, candidateWidth);
    }
    CGFloat panelWidth = MIN(MAX(measuredWidth, minWidth), maxWidth);
    CGFloat maxTextWidth = panelWidth - horizontalPadding * 2.0;
    CGFloat textLineHeight = 15.0;
    CGFloat headerHeight = showHeader ? 16.0 : 0.0;
    CGFloat headerGap = showHeader ? 6.0 : 0.0;
    NSMutableArray<NSNumber *> *rowHeights = [NSMutableArray arrayWithCapacity:measuredCount];
    CGFloat rowsTotalHeight = 0.0;
    for (NSUInteger index = 0; index < measuredCount; index += 1) {
        NSString *text = suzakuBridgeTakeCandidateLabel(index);
        if (text == nil) {
            text = @"";
        }
        NSString *prefixed = [NSString stringWithFormat:@"› %@", text];
        NSRect boundingRect = [prefixed boundingRectWithSize:NSMakeSize(maxTextWidth, textLineHeight * maxTextLines)
                                                     options:NSStringDrawingUsesLineFragmentOrigin | NSStringDrawingTruncatesLastVisibleLine
                                                  attributes:measureAttributes];
        CGFloat measuredHeight =
            MAX(rowHeight, MIN(ceil(boundingRect.size.height) + 6.0, rowHeight * maxTextLines));
        [rowHeights addObject:@(measuredHeight)];
        rowsTotalHeight += measuredHeight;
    }
    CGFloat panelHeight = verticalPadding * 2.0 + headerHeight + headerGap + rowsTotalHeight;
    NSRect frame = [suzakuCandidateCompanionPanel frame];
    frame.size.width = panelWidth;
    frame.size.height = panelHeight;
    [suzakuCandidateCompanionPanel setFrame:frame display:NO];

    if (suzakuCandidateCompanionHeader != nil) {
        [suzakuCandidateCompanionHeader setHidden:!showHeader];
        [suzakuCandidateCompanionHeader setFrame:NSMakeRect(
                                              horizontalPadding,
                                              panelHeight - verticalPadding - headerHeight,
                                              panelWidth - horizontalPadding * 2.0,
                                              headerHeight)];
    }

    CGFloat currentRowTop = panelHeight - verticalPadding - headerHeight - headerGap;
    for (NSUInteger index = 0; index < [suzakuCandidateCompanionButtons count]; index += 1) {
        NSButton *button = suzakuCandidateCompanionButtons[index];
        if (index >= candidateCount) {
            [button setHidden:YES];
            continue;
        }

        NSString *text = suzakuBridgeTakeCandidateLabel(index);
        if (text == nil) {
            text = @"";
        }

        CGFloat currentRowHeight = index < rowHeights.count ? rowHeights[index].doubleValue : rowHeight;
        [button setHidden:NO];
        [button setFrame:NSMakeRect(
                             horizontalPadding,
                             currentRowTop - currentRowHeight + 2.0,
                             panelWidth - horizontalPadding * 2.0,
                             currentRowHeight)];
        [button setTag:(NSInteger)index];
        if ([button isKindOfClass:[SuzakuCandidateButton class]]) {
            [(SuzakuCandidateButton *)button setSuzakuCandidateIndex:index];
        }
        [button setTarget:suzakuActiveController];
        [button setAction:@selector(suzakuChooseCandidateFromButton:)];
        [button setToolTip:text];
        if (index == suzakuCandidateCompanionSelectedIndex) {
            NSColor *selectedColor = suzakuColorFromPackedRGBA(
                suzaku_host_companion_row_selected_text_rgba8());
            NSFont *selectedFont = [NSFont boldSystemFontOfSize:13.0];
            [button setAttributedTitle:suzakuCandidateAttributedTitle(
                                            [NSString stringWithFormat:@"› %@", text],
                                            selectedColor,
                                            selectedFont)];
            [button setContentTintColor:selectedColor];
            [button setFont:selectedFont];
        } else if (index == suzakuCandidateCompanionHoveredIndex) {
            NSColor *hoverColor = suzakuColorFromPackedRGBA(
                suzaku_host_companion_row_hover_text_rgba8());
            NSFont *hoverFont = [NSFont systemFontOfSize:13.0 weight:NSFontWeightSemibold];
            [button setAttributedTitle:suzakuCandidateAttributedTitle(
                                            [NSString stringWithFormat:@"› %@", text],
                                            hoverColor,
                                            hoverFont)];
            [button setContentTintColor:hoverColor];
            [button setFont:hoverFont];
        } else {
            NSColor *normalColor = suzakuColorFromPackedRGBA(
                suzaku_host_companion_row_normal_text_rgba8());
            NSFont *normalFont = [NSFont systemFontOfSize:13.0 weight:NSFontWeightMedium];
            [button setAttributedTitle:suzakuCandidateAttributedTitle(
                                            [NSString stringWithFormat:@"  %@", text],
                                            normalColor,
                                            normalFont)];
            [button setContentTintColor:normalColor];
            [button setFont:normalFont];
        }
        currentRowTop -= currentRowHeight;
    }

    suzakuCandidateCompanionVisible = YES;
    suzakuPositionCandidateCompanionNearClient(suzakuActiveClient);
    [suzakuCandidateCompanionPanel orderFrontRegardless];
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

static void suzakuRefreshCandidateCompanion(void) {
    suzakuCandidateCompanionRefreshCount += 1;
    suzakuCandidateCompanionSelectedIndex = (NSUInteger)suzaku_host_ime_selected_index();
    suzakuCandidateCompanionVisible = suzaku_host_ime_candidate_count() > 0;
    suzakuCandidateCompanionPrimaryCandidate =
        suzakuBridgeTakeNSString(suzaku_host_ime_primary_candidate_utf8);
    suzakuUpdateCandidateCompanionPanel();
}

@implementation SuzakuInputController
- (instancetype)initWithServer:(IMKServer *)server delegate:(id)delegate client:(id)inputClient {
    self = [super initWithServer:server delegate:delegate client:inputClient];
    if (self != nil) {
        suzakuControllerInitCount += 1;
        suzakuActiveController = self;
    }
    return self;
}

- (void)activateServer:(id)sender {
    suzakuControllerActive = YES;
    suzakuControllerActivateCount += 1;
    suzakuActiveController = self;
    suzakuActiveClient = sender;
    suzaku_host_ime_activate();
    suzakuRefreshCandidateCompanion();
    [super activateServer:sender];
}

- (void)deactivateServer:(id)sender {
    suzakuControllerActive = NO;
    suzakuControllerDeactivateCount += 1;
    suzaku_host_ime_deactivate();
    suzakuCandidateCompanionVisible = NO;
    suzakuActiveClient = nil;
    suzakuHideCandidateCompanionPanel();
    [super deactivateServer:sender];
}

- (BOOL)inputText:(NSString *)string key:(NSInteger)keyCode modifiers:(NSUInteger)flags client:(id)sender {
    (void)keyCode;
    (void)flags;
    suzakuActiveController = self;
    suzakuActiveClient = sender;
    suzakuControllerInputCount += 1;
    suzakuControllerLastMarkedText = [string copy];
    const char *utf8 = [string UTF8String];
    if (utf8 == NULL) {
        return NO;
    }
    BOOL accepted = suzaku_host_ime_replace_marked_text_utf8(utf8);
    if (accepted) {
        suzakuApplyMarkedTextToClient(sender);
        suzakuRefreshCandidateCompanion();
    }
    return accepted;
}

- (void)doCommandBySelector:(SEL)selector client:(id)sender {
    if (selector == @selector(moveUp:)) {
        suzakuActiveClient = sender;
        suzaku_host_ime_move_selection(-1);
        suzakuApplyMarkedTextToClient(sender);
        suzakuRefreshCandidateCompanion();
        return;
    }
    if (selector == @selector(moveDown:)) {
        suzakuActiveClient = sender;
        suzaku_host_ime_move_selection(1);
        suzakuApplyMarkedTextToClient(sender);
        suzakuRefreshCandidateCompanion();
        return;
    }
    if (selector == @selector(cancelOperation:)) {
        suzakuActiveClient = sender;
        suzaku_host_ime_clear_marked_text();
        suzakuControllerLastMarkedText = nil;
        suzakuApplyMarkedTextToClient(sender);
        suzakuRefreshCandidateCompanion();
        return;
    }
    if (selector == @selector(insertNewline:)) {
        suzakuActiveClient = sender;
        if (suzaku_host_ime_commit_selected(true)) {
            suzakuControllerLastMarkedText = nil;
            suzakuCommitTextToClient(sender);
            suzakuRefreshCandidateCompanion();
            return;
        }
    }
    (void)sender;
}

- (void)suzakuChooseCandidateFromButton:(id)sender {
    if (![sender isKindOfClass:[NSButton class]]) {
        return;
    }

    NSUInteger index = (NSUInteger)[(NSButton *)sender tag];
    BOOL alreadySelected = index == suzakuCandidateCompanionSelectedIndex;
    suzakuCandidateCompanionHoveredIndex = index;
    suzaku_host_ime_select_candidate(index);
    if (suzakuActiveClient != nil) {
        suzakuApplyMarkedTextToClient(suzakuActiveClient);
    }
    suzakuRefreshCandidateCompanion();
    if (alreadySelected && suzaku_host_ime_commit_selected(true)) {
        suzakuControllerLastMarkedText = nil;
        suzakuCommitTextToClient(suzakuActiveClient);
        suzakuRefreshCandidateCompanion();
    }
}

- (void)commitComposition:(id)sender {
    suzakuControllerCommitCount += 1;
    suzakuControllerLastMarkedText = nil;
    suzakuActiveClient = sender;
    if (suzaku_host_ime_commit_selected(true)) {
        suzakuCommitTextToClient(sender);
    }
    suzakuRefreshCandidateCompanion();
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

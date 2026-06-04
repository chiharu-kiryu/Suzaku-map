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

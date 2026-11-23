#import <Cocoa/Cocoa.h>
#import <Carbon/Carbon.h>
#include <stdint.h>
#include <stdbool.h>

typedef void (*QnVoidCallback)(void);

typedef struct {
    uint64_t collection_behavior;
    bool is_key_window;
} QnPanelDiag;

static QnVoidCallback g_quickbar_esc_cb = NULL;
static QnVoidCallback g_space_changed_cb = NULL;
static QnVoidCallback g_hotkey_cb = NULL;
static id g_space_observer = nil;
static EventHotKeyRef g_hotkey_ref = NULL;
static EventHandlerRef g_hotkey_handler_ref = NULL;

@interface QuickNotePanel : NSPanel
@end

@implementation QuickNotePanel
- (BOOL)canBecomeKeyWindow {
    return YES;
}

- (BOOL)canBecomeMainWindow {
    return NO;
}

- (void)cancelOperation:(id)sender {
    (void)sender;
    if (g_quickbar_esc_cb != NULL) {
        g_quickbar_esc_cb();
    }
}

- (void)keyDown:(NSEvent *)event {
    if (event.keyCode == 53) {
        [self cancelOperation:nil];
        return;
    }
    [super keyDown:event];
}
@end

@interface QuickNoteToastPanel : NSPanel
@end

@implementation QuickNoteToastPanel
- (BOOL)canBecomeKeyWindow {
    return NO;
}

- (BOOL)canBecomeMainWindow {
    return NO;
}
@end

static QuickNotePanel *g_quickbar_panel = nil;
static QuickNoteToastPanel *g_toast_panel = nil;

static void qn_run_on_main_thread(dispatch_block_t block) {
    if ([NSThread isMainThread]) {
        block();
    } else {
        dispatch_sync(dispatch_get_main_queue(), block);
    }
}

static NSScreen *qn_active_screen(void) {
    NSPoint mouseLoc = [NSEvent mouseLocation];
    for (NSScreen *screen in [NSScreen screens]) {
        if (NSPointInRect(mouseLoc, screen.frame)) {
            return screen;
        }
    }
    NSScreen *mainScreen = [NSScreen mainScreen];
    if (mainScreen != nil) {
        return mainScreen;
    }
    return [[NSScreen screens] firstObject];
}

static NSView *qn_find_wkwebview(NSView *view) {
    if (view == nil) {
        return nil;
    }
    Class wkClass = NSClassFromString(@"WKWebView");
    if (wkClass != Nil && [view isKindOfClass:wkClass]) {
        return view;
    }
    for (NSView *sub in view.subviews) {
        NSView *found = qn_find_wkwebview(sub);
        if (found != nil) {
            return found;
        }
    }
    return nil;
}

static QnPanelDiag qn_read_diag(NSWindow *win) {
    QnPanelDiag diag;
    if (win == nil) {
        diag.collection_behavior = 0;
        diag.is_key_window = false;
        return diag;
    }
    diag.collection_behavior = (uint64_t)win.collectionBehavior;
    diag.is_key_window = (bool)win.isKeyWindow;
    return diag;
}

void qn_configure_quick_bar_panel(void *ns_window_ptr, QnVoidCallback on_esc, QnPanelDiag *out_diag) {
    if (ns_window_ptr == NULL) {
        return;
    }
    g_quickbar_esc_cb = on_esc;

    __block QnPanelDiag diag = {0, false};
    qn_run_on_main_thread(^{
        NSWindow *taoWin = (__bridge NSWindow *)ns_window_ptr;
        if (g_quickbar_panel == nil) {
            NSRect frame = taoWin.frame;
            if (frame.size.width < 100.0 || frame.size.height < 100.0) {
                frame.size = NSMakeSize(640.0, 420.0);
            }
            // Must pass NSWindowStyleMaskNonactivatingPanel at init time so WindowServer
            // registers non-activating key focus eligibility cleanly.
            g_quickbar_panel = [[QuickNotePanel alloc]
                initWithContentRect:frame
                          styleMask:(NSWindowStyleMaskNonactivatingPanel |
                                     NSWindowStyleMaskTitled |
                                     NSWindowStyleMaskFullSizeContentView)
                            backing:NSBackingStoreBuffered
                              defer:NO];
            g_quickbar_panel.titleVisibility = NSWindowTitleHidden;
            g_quickbar_panel.titlebarAppearsTransparent = YES;
            g_quickbar_panel.movableByWindowBackground = NO;
            g_quickbar_panel.releasedWhenClosed = NO;
            g_quickbar_panel.hasShadow = YES;
            [[g_quickbar_panel standardWindowButton:NSWindowCloseButton] setHidden:YES];
            [[g_quickbar_panel standardWindowButton:NSWindowMiniaturizeButton] setHidden:YES];
            [[g_quickbar_panel standardWindowButton:NSWindowZoomButton] setHidden:YES];

            g_quickbar_panel.floatingPanel = YES;
            g_quickbar_panel.level = NSFloatingWindowLevel;
            g_quickbar_panel.becomesKeyOnlyIfNeeded = NO;
            g_quickbar_panel.hidesOnDeactivate = NO;
            // D-0002: strictly fullScreenAuxiliary (256), never canJoinAllSpaces.
            g_quickbar_panel.collectionBehavior = NSWindowCollectionBehaviorFullScreenAuxiliary;

            NSView *content = taoWin.contentView;
            if (content != nil) {
                [taoWin setContentView:[[NSView alloc] initWithFrame:taoWin.frame]];
                content.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
                [g_quickbar_panel setContentView:content];
                [content setFrame:g_quickbar_panel.contentView.bounds];
            }
            [taoWin orderOut:nil];
        }
        diag = qn_read_diag(g_quickbar_panel);
    });

    if (out_diag != NULL) {
        *out_diag = diag;
    }
}

void qn_show_quick_bar_panel(void *ns_window_ptr, QnPanelDiag *out_diag) {
    (void)ns_window_ptr;
    __block QnPanelDiag diag = {0, false};
    qn_run_on_main_thread(^{
        if (g_quickbar_panel == nil) {
            return;
        }
        g_quickbar_panel.collectionBehavior = NSWindowCollectionBehaviorFullScreenAuxiliary;

        NSScreen *screen = qn_active_screen();
        if (screen != nil) {
            NSRect visible = screen.visibleFrame;
            NSRect frame = g_quickbar_panel.frame;
            if (frame.size.width < 100.0 || frame.size.height < 100.0) {
                frame.size = NSMakeSize(640.0, 420.0);
            }
            frame.origin.x = visible.origin.x + (visible.size.width - frame.size.width) * 0.5;
            frame.origin.y = visible.origin.y + (visible.size.height - frame.size.height) * 0.58;
            [g_quickbar_panel setFrame:frame display:NO];
        }

        [g_quickbar_panel makeKeyAndOrderFront:nil];
        if (!g_quickbar_panel.isKeyWindow) {
            [g_quickbar_panel orderFrontRegardless];
            [g_quickbar_panel makeKeyWindow];
        }

        NSView *webView = qn_find_wkwebview(g_quickbar_panel.contentView);
        if (webView != nil) {
            [g_quickbar_panel makeFirstResponder:webView];
        } else if (g_quickbar_panel.contentView != nil) {
            [g_quickbar_panel makeFirstResponder:g_quickbar_panel.contentView];
        }

        diag = qn_read_diag(g_quickbar_panel);
    });

    if (out_diag != NULL) {
        *out_diag = diag;
    }
}

void qn_hide_quick_bar_panel(void *ns_window_ptr, QnPanelDiag *out_diag) {
    (void)ns_window_ptr;
    __block QnPanelDiag diag = {0, false};
    qn_run_on_main_thread(^{
        if (g_quickbar_panel != nil) {
            [g_quickbar_panel orderOut:nil];
        }
        diag = qn_read_diag(g_quickbar_panel);
    });

    if (out_diag != NULL) {
        *out_diag = diag;
    }
}

void qn_configure_toast_panel(void *ns_window_ptr, QnPanelDiag *out_diag) {
    if (ns_window_ptr == NULL) {
        return;
    }

    __block QnPanelDiag diag = {0, false};
    qn_run_on_main_thread(^{
        NSWindow *taoWin = (__bridge NSWindow *)ns_window_ptr;
        if (g_toast_panel == nil) {
            NSRect frame = taoWin.frame;
            if (frame.size.width < 100.0 || frame.size.height < 30.0) {
                frame.size = NSMakeSize(380.0, 44.0);
            }
            g_toast_panel = [[QuickNoteToastPanel alloc]
                initWithContentRect:frame
                          styleMask:(NSWindowStyleMaskNonactivatingPanel |
                                     NSWindowStyleMaskBorderless)
                            backing:NSBackingStoreBuffered
                              defer:NO];
            g_toast_panel.releasedWhenClosed = NO;
            g_toast_panel.hasShadow = YES;
            g_toast_panel.floatingPanel = YES;
            g_toast_panel.level = NSFloatingWindowLevel;
            g_toast_panel.becomesKeyOnlyIfNeeded = YES;
            g_toast_panel.hidesOnDeactivate = NO;
            // D-0002: strictly fullScreenAuxiliary (256), never canJoinAllSpaces.
            g_toast_panel.collectionBehavior = NSWindowCollectionBehaviorFullScreenAuxiliary;

            NSView *content = taoWin.contentView;
            if (content != nil) {
                [taoWin setContentView:[[NSView alloc] initWithFrame:taoWin.frame]];
                content.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
                [g_toast_panel setContentView:content];
                [content setFrame:g_toast_panel.contentView.bounds];
            }
            [taoWin orderOut:nil];
        }
        diag = qn_read_diag(g_toast_panel);
    });

    if (out_diag != NULL) {
        *out_diag = diag;
    }
}

void qn_show_toast_panel(void *ns_window_ptr, QnPanelDiag *out_diag) {
    (void)ns_window_ptr;
    __block QnPanelDiag diag = {0, false};
    qn_run_on_main_thread(^{
        if (g_toast_panel == nil) {
            return;
        }
        g_toast_panel.collectionBehavior = NSWindowCollectionBehaviorFullScreenAuxiliary;

        NSScreen *screen = qn_active_screen();
        if (screen != nil) {
            NSRect visible = screen.visibleFrame;
            NSRect frame = g_toast_panel.frame;
            if (frame.size.width < 100.0 || frame.size.height < 30.0) {
                frame.size = NSMakeSize(380.0, 44.0);
            }
            frame.origin.x = visible.origin.x + (visible.size.width - frame.size.width) * 0.5;
            frame.origin.y = visible.origin.y + 20.0;
            [g_toast_panel setFrame:frame display:NO];
        }

        // Never call makeKeyAndOrderFront or activate app for Toast.
        [g_toast_panel orderFrontRegardless];
        diag = qn_read_diag(g_toast_panel);
    });

    if (out_diag != NULL) {
        *out_diag = diag;
    }
}

void qn_hide_toast_panel(void *ns_window_ptr, QnPanelDiag *out_diag) {
    (void)ns_window_ptr;
    __block QnPanelDiag diag = {0, false};
    qn_run_on_main_thread(^{
        if (g_toast_panel != nil) {
            [g_toast_panel orderOut:nil];
        }
        diag = qn_read_diag(g_toast_panel);
    });

    if (out_diag != NULL) {
        *out_diag = diag;
    }
}

void qn_register_space_observer(QnVoidCallback on_space_changed) {
    g_space_changed_cb = on_space_changed;
    if (g_space_observer != nil) {
        return;
    }

    NSNotificationCenter *center = [[NSWorkspace sharedWorkspace] notificationCenter];
    g_space_observer = [center addObserverForName:NSWorkspaceActiveSpaceDidChangeNotification
                                           object:nil
                                            queue:[NSOperationQueue mainQueue]
                                       usingBlock:^(__unused NSNotification *note) {
        if (g_space_changed_cb != NULL) {
            g_space_changed_cb();
        }
    }];
}

static OSStatus qn_carbon_hotkey_handler(
    __unused EventHandlerCallRef next_handler,
    EventRef event,
    __unused void *user_data
) {
    EventHotKeyID hot_key_id;
    OSStatus err = GetEventParameter(
        event,
        kEventParamDirectObject,
        typeEventHotKeyID,
        NULL,
        sizeof(hot_key_id),
        NULL,
        &hot_key_id
    );
    if (err == noErr && hot_key_id.signature == 'QNHk' && hot_key_id.id == 1) {
        if (g_hotkey_cb != NULL) {
            g_hotkey_cb();
        }
    }
    return noErr;
}

int32_t qn_register_carbon_hotkey(
    uint32_t key_code,
    uint32_t modifiers,
    QnVoidCallback on_hotkey
) {
    g_hotkey_cb = on_hotkey;

    if (g_hotkey_handler_ref == NULL) {
        EventTypeSpec event_type;
        event_type.eventClass = kEventClassKeyboard;
        event_type.eventKind = kEventHotKeyPressed;
        OSStatus install_status = InstallApplicationEventHandler(
            &qn_carbon_hotkey_handler,
            1,
            &event_type,
            NULL,
            &g_hotkey_handler_ref
        );
        if (install_status != noErr) {
            return (int32_t)install_status;
        }
    }

    if (g_hotkey_ref != NULL) {
        UnregisterEventHotKey(g_hotkey_ref);
        g_hotkey_ref = NULL;
    }

    EventHotKeyID hot_key_id;
    hot_key_id.signature = 'QNHk';
    hot_key_id.id = 1;

    OSStatus reg_status = RegisterEventHotKey(
        (UInt32)key_code,
        (UInt32)modifiers,
        hot_key_id,
        GetApplicationEventTarget(),
        0,
        &g_hotkey_ref
    );
    return (int32_t)reg_status;
}

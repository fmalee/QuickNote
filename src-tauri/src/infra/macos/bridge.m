#import <Cocoa/Cocoa.h>
#import <Carbon/Carbon.h>
#import <objc/runtime.h>
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

static QnPanelDiag qn_read_diag(NSWindow *win) {
    QnPanelDiag diag;
    diag.collection_behavior = (uint64_t)win.collectionBehavior;
    diag.is_key_window = (bool)win.isKeyWindow;
    return diag;
}

void qn_configure_quick_bar_panel(void *ns_window_ptr, QnVoidCallback on_esc, QnPanelDiag *out_diag) {
    if (ns_window_ptr == NULL) {
        return;
    }
    g_quickbar_esc_cb = on_esc;

    NSWindow *win = (__bridge NSWindow *)ns_window_ptr;
    if (![win isKindOfClass:[QuickNotePanel class]]) {
        object_setClass(win, [QuickNotePanel class]);
    }
    NSPanel *panel = (NSPanel *)win;
    panel.styleMask |= NSWindowStyleMaskNonactivatingPanel;
    panel.floatingPanel = YES;
    panel.level = NSFloatingWindowLevel;
    panel.becomesKeyOnlyIfNeeded = NO;
    panel.hidesOnDeactivate = NO;
    // D-0002: strictly fullScreenAuxiliary (256), never canJoinAllSpaces.
    panel.collectionBehavior = NSWindowCollectionBehaviorFullScreenAuxiliary;

    if (out_diag != NULL) {
        *out_diag = qn_read_diag(panel);
    }
}

void qn_show_quick_bar_panel(void *ns_window_ptr, QnPanelDiag *out_diag) {
    if (ns_window_ptr == NULL) {
        return;
    }
    NSPanel *panel = (__bridge NSPanel *)ns_window_ptr;
    panel.collectionBehavior = NSWindowCollectionBehaviorFullScreenAuxiliary;

    NSScreen *screen = qn_active_screen();
    if (screen != nil) {
        NSRect visible = screen.visibleFrame;
        NSRect frame = panel.frame;
        if (frame.size.width < 100.0 || frame.size.height < 100.0) {
            frame.size = NSMakeSize(640.0, 420.0);
        }
        frame.origin.x = visible.origin.x + (visible.size.width - frame.size.width) * 0.5;
        frame.origin.y = visible.origin.y + (visible.size.height - frame.size.height) * 0.58;
        [panel setFrame:frame display:NO];
    }

    [panel orderFrontRegardless];
    [panel makeKeyWindow];

    if (out_diag != NULL) {
        *out_diag = qn_read_diag(panel);
    }
}

void qn_hide_quick_bar_panel(void *ns_window_ptr, QnPanelDiag *out_diag) {
    if (ns_window_ptr == NULL) {
        return;
    }
    NSPanel *panel = (__bridge NSPanel *)ns_window_ptr;
    [panel orderOut:nil];

    if (out_diag != NULL) {
        *out_diag = qn_read_diag(panel);
    }
}

void qn_configure_toast_panel(void *ns_window_ptr, QnPanelDiag *out_diag) {
    if (ns_window_ptr == NULL) {
        return;
    }
    NSWindow *win = (__bridge NSWindow *)ns_window_ptr;
    if (![win isKindOfClass:[QuickNoteToastPanel class]]) {
        object_setClass(win, [QuickNoteToastPanel class]);
    }
    NSPanel *panel = (NSPanel *)win;
    panel.styleMask |= NSWindowStyleMaskNonactivatingPanel;
    panel.floatingPanel = YES;
    panel.level = NSFloatingWindowLevel;
    panel.becomesKeyOnlyIfNeeded = YES;
    panel.hidesOnDeactivate = NO;
    // D-0002: strictly fullScreenAuxiliary (256), never canJoinAllSpaces.
    panel.collectionBehavior = NSWindowCollectionBehaviorFullScreenAuxiliary;

    if (out_diag != NULL) {
        *out_diag = qn_read_diag(panel);
    }
}

void qn_show_toast_panel(void *ns_window_ptr, QnPanelDiag *out_diag) {
    if (ns_window_ptr == NULL) {
        return;
    }
    NSPanel *panel = (__bridge NSPanel *)ns_window_ptr;
    panel.collectionBehavior = NSWindowCollectionBehaviorFullScreenAuxiliary;

    NSScreen *screen = qn_active_screen();
    if (screen != nil) {
        NSRect visible = screen.visibleFrame;
        NSRect frame = panel.frame;
        if (frame.size.width < 100.0 || frame.size.height < 30.0) {
            frame.size = NSMakeSize(420.0, 56.0);
        }
        frame.origin.x = visible.origin.x + (visible.size.width - frame.size.width) * 0.5;
        frame.origin.y = visible.origin.y + 48.0;
        [panel setFrame:frame display:NO];
    }

    // Never call makeKeyWindow or activate app for Toast.
    [panel orderFrontRegardless];

    if (out_diag != NULL) {
        *out_diag = qn_read_diag(panel);
    }
}

void qn_hide_toast_panel(void *ns_window_ptr, QnPanelDiag *out_diag) {
    if (ns_window_ptr == NULL) {
        return;
    }
    NSPanel *panel = (__bridge NSPanel *)ns_window_ptr;
    [panel orderOut:nil];

    if (out_diag != NULL) {
        *out_diag = qn_read_diag(panel);
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

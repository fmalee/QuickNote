#import <Cocoa/Cocoa.h>
#import <Carbon/Carbon.h>
#import <WebKit/WebKit.h>
#import <objc/runtime.h>
#include <math.h>
#include <stdint.h>
#include <stdbool.h>
#include <stdio.h>
#include <unistd.h>

typedef void (*QnVoidCallback)(void);

typedef struct {
    uint64_t collection_behavior;
    bool is_key_window;
} QnPanelDiag;

static QnVoidCallback g_quickbar_esc_cb = NULL;
static QnVoidCallback g_quickbar_resign_cb = NULL;
static QnVoidCallback g_space_changed_cb = NULL;
static QnVoidCallback g_hotkey_cb = NULL;
static id g_quickbar_resign_observer = nil;
static id g_space_observer = nil;
static EventHotKeyRef g_hotkey_ref = NULL;
static EventHandlerRef g_hotkey_handler_ref = NULL;
static uint32_t g_registered_key_code = 0;
static uint32_t g_registered_modifiers = 0;

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
static BOOL g_has_saved_standard_frame = NO;
static NSRect g_saved_standard_frame;
static BOOL g_has_saved_standard_state = NO;
static NSWindowCollectionBehavior g_saved_standard_collection_behavior = NSWindowCollectionBehaviorFullScreenPrimary;
static BOOL g_saved_standard_hides_on_deactivate = NO;
static NSWindowStyleMask g_saved_standard_style_mask = 0;
static BOOL g_is_auxiliary_mode = NO;
static __weak NSWindow *g_aux_main_window = nil;
static uint8_t g_aux_width_percent = 45;
static id g_aux_space_observer = nil;
static BOOL g_installed_aux_key_override = NO;
static IMP g_orig_can_become_key_window = NULL;
static id g_pending_exit_fullscreen_observer = nil;
static uint64_t g_pending_exit_fullscreen_token = 0;
static id g_pending_exit_aux_space_observer = nil;
static uint64_t g_pending_exit_aux_token = 0;

@interface QnPdfNavigationDelegate : NSObject <WKNavigationDelegate>
@property (nonatomic, assign) BOOL finished;
@property (nonatomic, strong) NSError *navError;
@end

@implementation QnPdfNavigationDelegate
- (void)webView:(WKWebView *)webView didFinishNavigation:(WKNavigation *)navigation {
    (void)webView;
    (void)navigation;
    self.finished = YES;
}

- (void)webView:(WKWebView *)webView didFailNavigation:(WKNavigation *)navigation withError:(NSError *)error {
    (void)webView;
    (void)navigation;
    self.navError = error;
    self.finished = YES;
}

- (void)webView:(WKWebView *)webView didFailProvisionalNavigation:(WKNavigation *)navigation withError:(NSError *)error {
    (void)webView;
    (void)navigation;
    self.navError = error;
    self.finished = YES;
}
@end

static void qn_write_err(char *err_buf, size_t err_buf_len, const char *msg) {
    if (err_buf == NULL || err_buf_len == 0) {
        return;
    }
    if (msg == NULL) {
        err_buf[0] = '\0';
        return;
    }
    snprintf(err_buf, err_buf_len, "%s", msg);
}

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
        if (g_quickbar_resign_observer == nil) {
            g_quickbar_resign_observer = [[NSNotificationCenter defaultCenter]
                addObserverForName:NSWindowDidResignKeyNotification
                            object:g_quickbar_panel
                             queue:[NSOperationQueue mainQueue]
                        usingBlock:^(__unused NSNotification *note) {
                if (g_quickbar_resign_cb != NULL) {
                    g_quickbar_resign_cb();
                }
            }];
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
            g_toast_panel.opaque = NO;
            g_toast_panel.backgroundColor = [NSColor clearColor];
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
                content.wantsLayer = YES;
                if (content.layer != nil) {
                    content.layer.cornerRadius = 10.0;
                    content.layer.masksToBounds = YES;
                }
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

void qn_register_quick_bar_resign_handler(QnVoidCallback on_resign) {
    g_quickbar_resign_cb = on_resign;
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

    if (g_hotkey_ref != NULL &&
        g_registered_key_code == key_code &&
        g_registered_modifiers == modifiers) {
        g_hotkey_cb = on_hotkey;
        return noErr;
    }

    EventHotKeyID hot_key_id;
    hot_key_id.signature = 'QNHk';
    hot_key_id.id = 1;

    EventHotKeyRef new_ref = NULL;
    OSStatus reg_status = RegisterEventHotKey(
        (UInt32)key_code,
        (UInt32)modifiers,
        hot_key_id,
        GetApplicationEventTarget(),
        0,
        &new_ref
    );
    if (reg_status != noErr) {
        return (int32_t)reg_status;
    }

    if (g_hotkey_ref != NULL) {
        UnregisterEventHotKey(g_hotkey_ref);
    }
    g_hotkey_ref = new_ref;
    g_registered_key_code = key_code;
    g_registered_modifiers = modifiers;
    g_hotkey_cb = on_hotkey;
    return (int32_t)reg_status;
}

int32_t qn_render_html_to_pdf(
    const char *html_utf8,
    const char *dest_path_utf8,
    char *err_buf,
    size_t err_buf_len
) {
    if (html_utf8 == NULL || html_utf8[0] == '\0') {
        qn_write_err(err_buf, err_buf_len, "HTML document must not be empty");
        return -1;
    }
    if (dest_path_utf8 == NULL || dest_path_utf8[0] == '\0') {
        qn_write_err(err_buf, err_buf_len, "Destination path must not be empty");
        return -1;
    }

    NSString *html = [NSString stringWithUTF8String:html_utf8];
    NSString *destPath = [NSString stringWithUTF8String:dest_path_utf8];
    if (html == nil || destPath == nil) {
        qn_write_err(err_buf, err_buf_len, "Invalid UTF-8 input for PDF export");
        return -1;
    }

    __block int32_t result = 0;
    qn_run_on_main_thread(^{
        if (@available(macOS 11.0, *)) {
            WKWebViewConfiguration *config = [[WKWebViewConfiguration alloc] init];
            WKWebView *webView = [[WKWebView alloc] initWithFrame:NSMakeRect(0.0, 0.0, 794.0, 1123.0)
                                                    configuration:config];
            QnPdfNavigationDelegate *delegate = [[QnPdfNavigationDelegate alloc] init];
            webView.navigationDelegate = delegate;

            [webView loadHTMLString:html baseURL:nil];

            NSDate *loadDeadline = [NSDate dateWithTimeIntervalSinceNow:15.0];
            while (!delegate.finished && [loadDeadline timeIntervalSinceNow] > 0.0) {
                CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.02, true);
            }

            if (!delegate.finished) {
                webView.navigationDelegate = nil;
                qn_write_err(err_buf, err_buf_len, "Timed out waiting for WKWebView HTML navigation");
                result = -2;
                return;
            }

            if (delegate.navError != nil) {
                webView.navigationDelegate = nil;
                qn_write_err(err_buf, err_buf_len, delegate.navError.localizedDescription.UTF8String);
                result = -3;
                return;
            }

            __block BOOL pdfDone = NO;
            __block NSData *capturedPdf = nil;
            __block NSError *capturedErr = nil;

            WKPDFConfiguration *pdfConfig = [[WKPDFConfiguration alloc] init];
            [webView createPDFWithConfiguration:pdfConfig
                              completionHandler:^(NSData *pdfData, NSError *error) {
                capturedPdf = pdfData;
                capturedErr = error;
                pdfDone = YES;
            }];

            NSDate *pdfDeadline = [NSDate dateWithTimeIntervalSinceNow:15.0];
            while (!pdfDone && [pdfDeadline timeIntervalSinceNow] > 0.0) {
                CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.02, true);
            }

            webView.navigationDelegate = nil;

            if (!pdfDone) {
                qn_write_err(err_buf, err_buf_len, "Timed out waiting for WKWebView PDF generation");
                result = -4;
                return;
            }

            if (capturedErr != nil || capturedPdf == nil || capturedPdf.length == 0) {
                const char *msg = capturedErr != nil
                    ? capturedErr.localizedDescription.UTF8String
                    : "WKWebView produced empty PDF data";
                qn_write_err(err_buf, err_buf_len, msg);
                result = -5;
                return;
            }

            NSError *writeErr = nil;
            BOOL written = [capturedPdf writeToFile:destPath
                                            options:NSDataWritingAtomic
                                              error:&writeErr];
            if (!written) {
                const char *msg = writeErr != nil
                    ? writeErr.localizedDescription.UTF8String
                    : "Failed to write PDF file";
                qn_write_err(err_buf, err_buf_len, msg);
                result = -6;
                return;
            }

            result = 0;
        } else {
            qn_write_err(err_buf, err_buf_len, "PDF export requires macOS 11.0 or later");
            result = -7;
        }
    });

    return result;
}

static void qn_cancel_pending_exit_fullscreen_observer(void) {
    g_pending_exit_fullscreen_token++;
    if (g_pending_exit_fullscreen_observer != nil) {
        [[NSNotificationCenter defaultCenter] removeObserver:g_pending_exit_fullscreen_observer];
        g_pending_exit_fullscreen_observer = nil;
    }
}

static void qn_cancel_pending_exit_aux_observer(void) {
    g_pending_exit_aux_token++;
    if (g_pending_exit_aux_space_observer != nil) {
        [[[NSWorkspace sharedWorkspace] notificationCenter] removeObserver:g_pending_exit_aux_space_observer];
        g_pending_exit_aux_space_observer = nil;
    }
}

static NSWindowCollectionBehavior qn_auxiliary_collection_behavior(void) {
    NSWindowCollectionBehavior behavior =
        (NSWindowCollectionBehaviorCanJoinAllSpaces |
         NSWindowCollectionBehaviorFullScreenAuxiliary |
         NSWindowCollectionBehaviorStationary);
    behavior &= ~NSWindowCollectionBehaviorMoveToActiveSpace;
    return behavior;
}

static void qn_save_standard_window_state(NSWindow *win, BOOL isFullScreen) {
    if (win == nil || g_is_auxiliary_mode || isFullScreen) {
        return;
    }
    g_saved_standard_frame = win.frame;
    g_has_saved_standard_frame = YES;
    g_saved_standard_collection_behavior = win.collectionBehavior;
    g_saved_standard_hides_on_deactivate = win.hidesOnDeactivate;
    g_saved_standard_style_mask = win.styleMask;
    g_has_saved_standard_state = YES;
}

static BOOL qn_aux_can_become_key_window(id self, SEL _cmd) {
    if (g_is_auxiliary_mode && self == g_aux_main_window) {
        return YES;
    }
    if (g_orig_can_become_key_window != NULL) {
        return ((BOOL (*)(id, SEL))g_orig_can_become_key_window)(self, _cmd);
    }
    return NO;
}

static void qn_ensure_aux_key_window_override(NSWindow *win) {
    if (win == nil || g_installed_aux_key_override) {
        return;
    }
    Class cls = [win class];
    Method m = class_getInstanceMethod(cls, @selector(canBecomeKeyWindow));
    if (m != NULL) {
        g_orig_can_become_key_window = method_setImplementation(m, (IMP)qn_aux_can_become_key_window);
    }
    g_installed_aux_key_override = YES;
}

static BOOL qn_is_fullscreen_space_on_screen(NSScreen *screen) {
    if (screen == nil) {
        return NO;
    }
    NSScreen *primary = [[NSScreen screens] firstObject];
    if (primary == nil) {
        return NO;
    }
    CGFloat primaryHeight = primary.frame.size.height;
    NSRect sf = screen.frame;
    CGRect cgScreen = CGRectMake(
        sf.origin.x,
        primaryHeight - (sf.origin.y + sf.size.height),
        sf.size.width,
        sf.size.height
    );

    CFArrayRef list = CGWindowListCopyWindowInfo(
        kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
        kCGNullWindowID
    );
    if (list == NULL) {
        return NO;
    }
    NSArray *windows = (__bridge_transfer NSArray *)list;
    pid_t selfPid = getpid();
    for (NSDictionary *info in windows) {
        NSNumber *layerNum = info[(id)kCGWindowLayer];
        if (layerNum == nil || [layerNum integerValue] != 0) {
            continue;
        }
        NSNumber *pidNum = info[(id)kCGWindowOwnerPID];
        if (pidNum != nil && (pid_t)[pidNum intValue] == selfPid) {
            continue;
        }
        NSDictionary *boundsDict = info[(id)kCGWindowBounds];
        if (boundsDict == nil) {
            continue;
        }
        CGRect bounds = CGRectZero;
        if (!CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)boundsDict, &bounds)) {
            continue;
        }
        if (fabs(bounds.origin.x - cgScreen.origin.x) < 2.0 &&
            fabs(bounds.origin.y - cgScreen.origin.y) < 2.0 &&
            fabs(bounds.size.width - cgScreen.size.width) < 2.0 &&
            fabs(bounds.size.height - cgScreen.size.height) < 2.0) {
            return YES;
        }
    }
    return NO;
}

static NSRect qn_compute_auxiliary_frame(NSScreen *screen, uint8_t width_percent) {
    uint8_t clamped = width_percent < 30
        ? 30
        : (width_percent > 60 ? 60 : width_percent);
    NSRect frame = screen.frame;
    NSRect visible = screen.visibleFrame;
    CGFloat w = round(visible.size.width * ((CGFloat)clamped / 100.0));
    if (w < 480.0) {
        w = 480.0;
    }
    if (w > visible.size.width) {
        w = visible.size.width;
    }
    CGFloat x = frame.origin.x + (frame.size.width - w);
    CGFloat top = frame.origin.y + frame.size.height;
    CGFloat bottom = visible.origin.y;
    CGFloat visibleTop = visible.origin.y + visible.size.height;
    if (top > visibleTop && !qn_is_fullscreen_space_on_screen(screen)) {
        top = visibleTop;
    }
    return NSMakeRect(x, bottom, w, top - bottom);
}

static void qn_ensure_aux_space_observer(void) {
    if (g_aux_space_observer != nil) {
        return;
    }
    NSNotificationCenter *center = [[NSWorkspace sharedWorkspace] notificationCenter];
    g_aux_space_observer = [center addObserverForName:NSWorkspaceActiveSpaceDidChangeNotification
                                               object:nil
                                                queue:[NSOperationQueue mainQueue]
                                           usingBlock:^(__unused NSNotification *note) {
        if (!g_is_auxiliary_mode) {
            return;
        }
        NSWindow *win = g_aux_main_window;
        if (win == nil || !win.isVisible) {
            return;
        }
        NSScreen *screen = win.screen != nil ? win.screen : qn_active_screen();
        if (screen == nil) {
            return;
        }
        NSRect target = qn_compute_auxiliary_frame(screen, g_aux_width_percent);
        [win setFrame:target display:YES animate:NO];
    }];
}

static void qn_apply_auxiliary_docking(NSWindow *win, uint8_t auxiliary_width_percent) {
    if (win == nil) {
        return;
    }
    g_is_auxiliary_mode = YES;
    g_aux_main_window = win;
    g_aux_width_percent = auxiliary_width_percent;
    qn_ensure_aux_key_window_override(win);
    qn_ensure_aux_space_observer();

    // Detach from any previous Space before switching collectionBehavior so
    // WindowServer presents the sidebar directly over the active fullscreen Space
    // without pulling the user back to the desktop Space.
    if (win.isVisible) {
        [win orderOut:nil];
    }
    if (win.isMiniaturized) {
        [win deminiaturize:nil];
    }

    // Keep the app in Accessory policy while presenting the auxiliary sidebar so
    // activating for keyboard input over fullscreen apps never switches Spaces.
    if ([NSApp activationPolicy] != NSApplicationActivationPolicyAccessory) {
        [NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];
    }

    // Remove NSWindowStyleMaskTitled so AppKit's constrainFrameRect:toScreen: does
    // not clamp the sidebar top edge to visibleFrame inside fullscreen Spaces.
    win.styleMask = (win.styleMask & ~NSWindowStyleMaskTitled);
    win.collectionBehavior = qn_auxiliary_collection_behavior();
    win.level = NSFloatingWindowLevel;
    win.hidesOnDeactivate = NO;
    win.hasShadow = YES;

    NSScreen *screen = qn_active_screen();
    if (screen != nil) {
        NSRect target = qn_compute_auxiliary_frame(screen, auxiliary_width_percent);
        [win setFrame:target display:NO animate:NO];
    }

    [win orderFrontRegardless];
    [NSApp activateIgnoringOtherApps:YES];
    [win makeKeyAndOrderFront:nil];

    NSView *webView = qn_find_wkwebview(win.contentView);
    if (webView != nil) {
        [win makeFirstResponder:webView];
    } else if (win.contentView != nil) {
        [win makeFirstResponder:win.contentView];
    }
}

static void qn_enter_auxiliary_mode_locked(NSWindow *win, uint8_t auxiliary_width_percent) {
    if (win == nil) {
        return;
    }
    BOOL isFullScreen = (win.styleMask & NSWindowStyleMaskFullScreen) != 0;
    qn_cancel_pending_exit_fullscreen_observer();
    qn_cancel_pending_exit_aux_observer();
    qn_save_standard_window_state(win, isFullScreen);

    if (isFullScreen) {
        g_is_auxiliary_mode = YES;
        uint64_t token = ++g_pending_exit_fullscreen_token;
        __weak NSWindow *weakWin = win;
        g_pending_exit_fullscreen_observer = [[NSNotificationCenter defaultCenter]
            addObserverForName:NSWindowDidExitFullScreenNotification
                        object:win
                         queue:[NSOperationQueue mainQueue]
                    usingBlock:^(NSNotification *note) {
                        (void)note;
                        if (token != g_pending_exit_fullscreen_token) {
                            return;
                        }
                        qn_cancel_pending_exit_fullscreen_observer();
                        NSWindow *strongWin = weakWin;
                        if (strongWin != nil) {
                            // Now that native fullscreen exit has finished, capture the restored
                            // standard window frame and styleMask before entering auxiliary mode.
                            g_is_auxiliary_mode = NO;
                            qn_save_standard_window_state(strongWin, NO);
                            qn_apply_auxiliary_docking(strongWin, auxiliary_width_percent);
                        }
                    }];
        dispatch_after(
            dispatch_time(DISPATCH_TIME_NOW, (int64_t)(3 * NSEC_PER_SEC)),
            dispatch_get_main_queue(),
            ^{
                if (token == g_pending_exit_fullscreen_token) {
                    qn_cancel_pending_exit_fullscreen_observer();
                }
            }
        );
        [win toggleFullScreen:nil];
    } else {
        qn_apply_auxiliary_docking(win, auxiliary_width_percent);
    }
}

static NSRect qn_clamp_rect_to_visible(NSRect rect, NSScreen *screen) {
    if (screen == nil) {
        return rect;
    }
    NSRect visible = screen.visibleFrame;
    CGFloat w = MIN(rect.size.width, visible.size.width);
    CGFloat h = MIN(rect.size.height, visible.size.height);
    CGFloat x = MAX(visible.origin.x, MIN(rect.origin.x, visible.origin.x + visible.size.width - w));
    CGFloat y = MAX(visible.origin.y, MIN(rect.origin.y, visible.origin.y + visible.size.height - h));
    return NSMakeRect(x, y, w, h);
}

static void qn_exit_auxiliary_mode_locked(NSWindow *win) {
    if (win == nil) {
        return;
    }
    qn_cancel_pending_exit_fullscreen_observer();
    qn_cancel_pending_exit_aux_observer();

    NSScreen *currentScreen = win.screen != nil ? win.screen : qn_active_screen();
    BOOL wasAuxiliaryVisible = g_is_auxiliary_mode && win.isVisible;
    BOOL inFullscreenSpace = wasAuxiliaryVisible && qn_is_fullscreen_space_on_screen(currentScreen);

    NSWindowCollectionBehavior restoredBehavior = g_has_saved_standard_state
        ? g_saved_standard_collection_behavior
        : NSWindowCollectionBehaviorFullScreenPrimary;
    restoredBehavior &= ~(NSWindowCollectionBehaviorCanJoinAllSpaces |
                          NSWindowCollectionBehaviorFullScreenAuxiliary |
                          NSWindowCollectionBehaviorStationary |
                          NSWindowCollectionBehaviorMoveToActiveSpace);
    if (restoredBehavior == NSWindowCollectionBehaviorDefault) {
        restoredBehavior = NSWindowCollectionBehaviorFullScreenPrimary;
    }

    NSWindowStyleMask savedStyleMask = g_has_saved_standard_state ? g_saved_standard_style_mask : 0;
    BOOL savedHidesOnDeactivate = g_has_saved_standard_state ? g_saved_standard_hides_on_deactivate : NO;
    BOOL shouldRestoreFrame = g_is_auxiliary_mode && g_has_saved_standard_frame;
    NSRect savedFrame = g_saved_standard_frame;

    g_is_auxiliary_mode = NO;
    g_has_saved_standard_state = NO;
    g_has_saved_standard_frame = NO;
    g_saved_standard_style_mask = 0;

    if (inFullscreenSpace) {
        // Never strip CanJoinAllSpaces while the other app's fullscreen Space is still active,
        // otherwise WindowServer binds this window to that fullscreen Space and later
        // makeKeyAndOrderFront: pulls the screen back into it.
        // Keep CanJoinAllSpaces (without FullScreenAuxiliary) while hidden, switch to Regular
        // so macOS transitions back to the desktop Space, then present on the desktop Space
        // first and strip CanJoinAllSpaces only once the window is visible on the desktop Space.
        [win orderOut:nil];
        win.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces;
        if (savedStyleMask != 0) {
            win.styleMask = savedStyleMask;
        } else {
            win.styleMask = (win.styleMask | NSWindowStyleMaskTitled);
        }
        win.level = NSNormalWindowLevel;
        win.hidesOnDeactivate = savedHidesOnDeactivate;

        uint64_t token = ++g_pending_exit_aux_token;
        __weak NSWindow *weakWin = win;
        void (^presentOnDesktop)(void) = ^{
            if (token != g_pending_exit_aux_token || g_is_auxiliary_mode) {
                return;
            }
            qn_cancel_pending_exit_aux_observer();
            NSWindow *strongWin = weakWin;
            if (strongWin == nil) {
                return;
            }
            NSScreen *screen = qn_active_screen();
            NSRect targetFrame = shouldRestoreFrame ? savedFrame : strongWin.frame;
            NSRect restored = qn_clamp_rect_to_visible(targetFrame, screen);
            [strongWin setFrame:restored display:NO animate:NO];
            [NSApp activateIgnoringOtherApps:YES];
            [strongWin makeKeyAndOrderFront:nil];
            // Now that the window is on screen in the desktop Space, strip CanJoinAllSpaces
            // so WindowServer permanently anchors it to this desktop Space.
            strongWin.collectionBehavior = restoredBehavior;
        };

        g_pending_exit_aux_space_observer = [[[NSWorkspace sharedWorkspace] notificationCenter]
            addObserverForName:NSWorkspaceActiveSpaceDidChangeNotification
                        object:nil
                         queue:[NSOperationQueue mainQueue]
                    usingBlock:^(__unused NSNotification *note) {
                        dispatch_after(
                            dispatch_time(DISPATCH_TIME_NOW, (int64_t)(50 * NSEC_PER_MSEC)),
                            dispatch_get_main_queue(),
                            ^{
                                presentOnDesktop();
                            }
                        );
                    }];

        if ([NSApp activationPolicy] != NSApplicationActivationPolicyRegular) {
            [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
        }
        [NSApp activateIgnoringOtherApps:YES];

        dispatch_after(
            dispatch_time(DISPATCH_TIME_NOW, (int64_t)(600 * NSEC_PER_MSEC)),
            dispatch_get_main_queue(),
            ^{
                presentOnDesktop();
            }
        );
        return;
    }

    if (savedStyleMask != 0) {
        win.styleMask = savedStyleMask;
    } else {
        win.styleMask = (win.styleMask | NSWindowStyleMaskTitled);
    }
    win.level = NSNormalWindowLevel;
    win.collectionBehavior = restoredBehavior;
    win.hidesOnDeactivate = savedHidesOnDeactivate;

    if (shouldRestoreFrame || wasAuxiliaryVisible) {
        NSRect targetFrame = shouldRestoreFrame ? savedFrame : win.frame;
        NSRect restored = qn_clamp_rect_to_visible(targetFrame, currentScreen);
        [win setFrame:restored display:YES animate:YES];
    }

    if (wasAuxiliaryVisible) {
        if ([NSApp activationPolicy] != NSApplicationActivationPolicyRegular) {
            [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
        }
        [NSApp activateIgnoringOtherApps:YES];
        [win makeKeyAndOrderFront:nil];
    }
}

void qn_enter_auxiliary_sidebar_mode(
    void *ns_window_ptr,
    uint8_t auxiliary_width_percent,
    QnPanelDiag *out_diag
) {
    if (ns_window_ptr == NULL) {
        return;
    }

    __block QnPanelDiag diag = {0, false};
    qn_run_on_main_thread(^{
        NSWindow *win = (__bridge NSWindow *)ns_window_ptr;
        if (win == nil) {
            return;
        }
        qn_enter_auxiliary_mode_locked(win, auxiliary_width_percent);
        diag = qn_read_diag(win);
    });

    if (out_diag != NULL) {
        *out_diag = diag;
    }
}

void qn_exit_auxiliary_sidebar_mode(
    void *ns_window_ptr,
    QnPanelDiag *out_diag
) {
    if (ns_window_ptr == NULL) {
        return;
    }

    __block QnPanelDiag diag = {0, false};
    qn_run_on_main_thread(^{
        NSWindow *win = (__bridge NSWindow *)ns_window_ptr;
        if (win == nil) {
            return;
        }
        qn_exit_auxiliary_mode_locked(win);
        diag = qn_read_diag(win);
    });

    if (out_diag != NULL) {
        *out_diag = diag;
    }
}

void qn_set_main_window_mode(
    void *ns_window_ptr,
    uint8_t mode,
    uint8_t auxiliary_width_percent,
    QnPanelDiag *out_diag
) {
    if (ns_window_ptr == NULL) {
        return;
    }

    __block QnPanelDiag diag = {0, false};
    qn_run_on_main_thread(^{
        NSWindow *win = (__bridge NSWindow *)ns_window_ptr;
        if (win == nil) {
            return;
        }

        if (mode == 0) {
            qn_exit_auxiliary_mode_locked(win);
        } else if (mode == 1) {
            qn_cancel_pending_exit_fullscreen_observer();
            win.level = NSNormalWindowLevel;
            win.collectionBehavior = NSWindowCollectionBehaviorFullScreenPrimary;
            g_is_auxiliary_mode = NO;
            g_has_saved_standard_state = NO;
        } else if (mode == 2) {
            qn_enter_auxiliary_mode_locked(win, auxiliary_width_percent);
        }

        diag = qn_read_diag(win);
    });

    if (out_diag != NULL) {
        *out_diag = diag;
    }
}

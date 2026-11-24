import { describe, expect, it } from 'vitest';
import { MergeView } from '@codemirror/merge';
import { EditorState } from '@codemirror/state';
import { EditorView } from '@codemirror/view';

function installMinimalDomShim() {
  class MiniNode {
    nodeType: number;
    nodeName: string;
    childNodes: MiniNode[] = [];
    parentNode: MiniNode | null = null;
    ownerDocument: unknown = null;
    textContent: string | null = '';

    constructor(nodeType: number, nodeName: string) {
      this.nodeType = nodeType;
      this.nodeName = nodeName;
    }

    get firstChild(): MiniNode | null {
      return this.childNodes[0] ?? null;
    }
    get lastChild(): MiniNode | null {
      return this.childNodes[this.childNodes.length - 1] ?? null;
    }
    get nextSibling(): MiniNode | null {
      if (!this.parentNode) return null;
      const idx = this.parentNode.childNodes.indexOf(this);
      return idx >= 0 ? (this.parentNode.childNodes[idx + 1] ?? null) : null;
    }
    get previousSibling(): MiniNode | null {
      if (!this.parentNode) return null;
      const idx = this.parentNode.childNodes.indexOf(this);
      return idx > 0 ? (this.parentNode.childNodes[idx - 1] ?? null) : null;
    }
    get isConnected() {
      return true;
    }

    appendChild<T extends MiniNode>(child: T): T {
      return this.insertBefore(child, null);
    }

    insertBefore<T extends MiniNode>(child: T, ref: MiniNode | null): T {
      if (child.parentNode) {
        child.parentNode.removeChild(child);
      }
      if (ref) {
        const idx = this.childNodes.indexOf(ref);
        if (idx >= 0) {
          this.childNodes.splice(idx, 0, child);
        } else {
          this.childNodes.push(child);
        }
      } else {
        this.childNodes.push(child);
      }
      child.parentNode = this;
      return child;
    }

    removeChild<T extends MiniNode>(child: T): T {
      const idx = this.childNodes.indexOf(child);
      if (idx >= 0) {
        this.childNodes.splice(idx, 1);
      }
      child.parentNode = null;
      return child;
    }

    remove() {
      this.parentNode?.removeChild(this);
    }

    contains(other: MiniNode | null): boolean {
      let cur = other;
      while (cur) {
        if (cur === this) return true;
        cur = cur.parentNode;
      }
      return false;
    }

    cloneNode(deep?: boolean): MiniNode {
      const copy = new MiniNode(this.nodeType, this.nodeName);
      copy.textContent = this.textContent;
      if (deep) {
        for (const ch of this.childNodes) {
          copy.appendChild(ch.cloneNode(true));
        }
      }
      return copy;
    }
  }

  class MiniElement extends MiniNode {
    tagName: string;
    className = '';
    style: Record<string, string> = {};
    attributes = new Map<string, string>();
    listeners = new Map<string, Set<(...args: unknown[]) => void>>();
    tabIndex = -1;
    spellcheck = false;
    contentEditable = 'inherit';
    scrollTop = 0;
    scrollLeft = 0;
    clientWidth = 600;
    clientHeight = 400;
    offsetWidth = 600;
    offsetHeight = 400;

    constructor(tagName: string) {
      super(1, tagName.toUpperCase());
      this.tagName = tagName.toUpperCase();
    }

    get children() {
      return this.childNodes.filter((n): n is MiniElement => n instanceof MiniElement);
    }

    get classList() {
      const tokens = () => this.className.split(/\s+/).filter(Boolean);
      return {
        add: (...cls: string[]) => {
          const set = new Set(tokens());
          cls.forEach((c) => set.add(c));
          this.className = Array.from(set).join(' ');
        },
        remove: (...cls: string[]) => {
          const set = new Set(tokens());
          cls.forEach((c) => set.delete(c));
          this.className = Array.from(set).join(' ');
        },
        contains: (c: string) => tokens().includes(c),
        toggle: (c: string, force?: boolean) => {
          const has = tokens().includes(c);
          const next = force ?? !has;
          if (next) {
            const set = new Set(tokens());
            set.add(c);
            this.className = Array.from(set).join(' ');
          } else {
            this.className = tokens()
              .filter((t) => t !== c)
              .join(' ');
          }
          return next;
        },
      };
    }

    setAttribute(k: string, v: string) {
      this.attributes.set(k, String(v));
      if (k === 'class') this.className = String(v);
    }
    getAttribute(k: string) {
      if (k === 'class') return this.className || null;
      return this.attributes.get(k) ?? null;
    }
    removeAttribute(k: string) {
      this.attributes.delete(k);
    }
    hasAttribute(k: string) {
      return this.attributes.has(k);
    }
    addEventListener(type: string, cb: (...args: unknown[]) => void) {
      if (!this.listeners.has(type)) this.listeners.set(type, new Set());
      this.listeners.get(type)!.add(cb);
    }
    removeEventListener(type: string, cb: (...args: unknown[]) => void) {
      this.listeners.get(type)?.delete(cb);
    }
    focus() {}
    blur() {}
    getBoundingClientRect() {
      return { top: 0, left: 0, right: 600, bottom: 400, width: 600, height: 400 };
    }
    getClientRects() {
      return [];
    }
    querySelector(selector: string): MiniElement | null {
      const cls = selector.startsWith('.') ? selector.slice(1) : null;
      for (const ch of this.children) {
        if (cls && ch.classList.contains(cls)) return ch;
        const found = ch.querySelector(selector);
        if (found) return found;
      }
      return null;
    }
    querySelectorAll(selector: string): MiniElement[] {
      const cls = selector.startsWith('.') ? selector.slice(1) : null;
      const out: MiniElement[] = [];
      for (const ch of this.children) {
        if (cls && ch.classList.contains(cls)) out.push(ch);
        out.push(...ch.querySelectorAll(selector));
      }
      return out;
    }
    set innerHTML(val: string) {
      this.childNodes = [];
      this.textContent = val;
    }
    get innerHTML() {
      return this.textContent ?? '';
    }
  }

  const head = new MiniElement('head');
  const body = new MiniElement('body');
  const docEl = new MiniElement('html');
  docEl.appendChild(head);
  docEl.appendChild(body);

  const doc = {
    nodeType: 9,
    head,
    body,
    documentElement: docEl,
    activeElement: body,
    defaultView: null as unknown,
    createElement: (tag: string) => {
      const el = new MiniElement(tag);
      el.ownerDocument = doc;
      return el;
    },
    createElementNS: (_ns: string, tag: string) => {
      const el = new MiniElement(tag);
      el.ownerDocument = doc;
      return el;
    },
    createTextNode: (text: string) => {
      const n = new MiniNode(3, '#text');
      n.textContent = text;
      n.ownerDocument = doc;
      return n;
    },
    createDocumentFragment: () => new MiniNode(11, '#document-fragment'),
    createRange: () => ({
      setStart: () => {},
      setEnd: () => {},
      collapse: () => {},
      getBoundingClientRect: () => ({ top: 0, left: 0, right: 0, bottom: 0, width: 0, height: 0 }),
      getClientRects: () => [],
    }),
    getSelection: () => null,
    hasFocus: () => false,
    createComment: (_text: string) => new MiniNode(8, '#comment'),
    addEventListener: () => {},
    removeEventListener: () => {},
    querySelector: (sel: string) => docEl.querySelector(sel),
    querySelectorAll: (sel: string) => docEl.querySelectorAll(sel),
  };

  const win = {
    document: doc,
    setTimeout,
    clearTimeout,
    getComputedStyle: () => ({
      getPropertyValue: () => '',
      fontSize: '14px',
      lineHeight: '20px',
      fontFamily: 'sans-serif',
      whiteSpace: 'pre-wrap',
      tabSize: '2',
    }),
    addEventListener: () => {},
    removeEventListener: () => {},
    requestAnimationFrame: (cb: FrameRequestCallback) =>
      setTimeout(() => {
        if ((globalThis as Record<string, unknown>).document === doc) {
          cb(Date.now());
        }
      }, 0),
    cancelAnimationFrame: (id: number) => clearTimeout(id),
    matchMedia: () => ({
      matches: false,
      addEventListener: () => {},
      removeEventListener: () => {},
    }),
  };
  doc.defaultView = win;

  const g = globalThis as Record<string, unknown>;
  const prev = {
    document: g.document,
    window: g.window,
    Window: g.Window,
    HTMLElement: g.HTMLElement,
    Node: g.Node,
    MutationObserver: g.MutationObserver,
    ResizeObserver: g.ResizeObserver,
    getComputedStyle: g.getComputedStyle,
    requestAnimationFrame: g.requestAnimationFrame,
    cancelAnimationFrame: g.cancelAnimationFrame,
  };

  g.document = doc;
  g.window = win;
  g.Window = class {};
  g.HTMLElement = MiniElement;
  g.Node = MiniNode;
  g.getComputedStyle = win.getComputedStyle;
  g.requestAnimationFrame = win.requestAnimationFrame;
  g.cancelAnimationFrame = win.cancelAnimationFrame;
  g.MutationObserver = class {
    observe() {}
    disconnect() {}
    takeRecords() {
      return [];
    }
  };
  g.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  };

  return {
    parent: doc.createElement('div') as unknown as HTMLElement,
    restore: () => {
      Object.assign(g, prev);
    },
  };
}

describe('Real @codemirror/merge MergeView smoke (M4-audit)', () => {
  it('creates .cm-mergeView dual panes with real MergeView and destroys cleanly', () => {
    const { parent, restore } = installMinimalDomShim();
    try {
      const mv = new MergeView({
        parent,
        a: {
          doc: '# Title\n\nDisk line 1\nShared line\n',
          extensions: [EditorState.readOnly.of(true)],
        },
        b: {
          doc: '# Title\n\nLocal draft line 1\nShared line\n',
          extensions: [EditorView.editable.of(true)],
        },
        revertControls: 'a-to-b',
        highlightChanges: true,
        gutter: true,
      });

      const root = parent.querySelector('.cm-mergeView');
      expect(root).not.toBeNull();
      expect(mv.a.state.doc.toString()).toContain('Disk line 1');
      expect(mv.b.state.doc.toString()).toContain('Local draft line 1');

      expect(() => mv.destroy()).not.toThrow();
    } finally {
      restore();
    }
  });
});

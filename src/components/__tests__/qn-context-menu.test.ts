import fs from 'node:fs';
import path from 'node:path';
import * as vueRuntime from 'vue';
import { createRenderer, defineComponent, h, nextTick, ssrContextKey } from 'vue';
import { compileScript, compileTemplate, parse } from 'vue/compiler-sfc';
import { describe, expect, it, vi } from 'vitest';
import QnContextMenu, { type ContextMenuEntry } from '../QnContextMenu.vue';

type Listener = (event: unknown) => void;

class FakeNode {
  parentNode: FakeNode | null = null;
  childNodes: FakeNode[] = [];
  listeners = new Map<string, Set<Listener>>();

  insertBefore(child: FakeNode, anchor: FakeNode | null): FakeNode {
    if (child.parentNode) {
      child.parentNode.removeChild(child);
    }
    child.parentNode = this;
    if (!anchor) {
      this.childNodes.push(child);
    } else {
      const idx = this.childNodes.indexOf(anchor);
      if (idx >= 0) {
        this.childNodes.splice(idx, 0, child);
      } else {
        this.childNodes.push(child);
      }
    }
    return child;
  }

  removeChild(child: FakeNode): FakeNode {
    const idx = this.childNodes.indexOf(child);
    if (idx >= 0) {
      this.childNodes.splice(idx, 1);
      child.parentNode = null;
    }
    return child;
  }

  get nextSibling(): FakeNode | null {
    if (!this.parentNode) return null;
    const siblings = this.parentNode.childNodes;
    const idx = siblings.indexOf(this);
    return idx >= 0 && idx + 1 < siblings.length ? (siblings[idx + 1] ?? null) : null;
  }

  addEventListener(type: string, fn: Listener): void {
    let set = this.listeners.get(type);
    if (!set) {
      set = new Set();
      this.listeners.set(type, set);
    }
    set.add(fn);
  }

  removeEventListener(type: string, fn: Listener): void {
    this.listeners.get(type)?.delete(fn);
  }

  dispatchEvent(event: Record<string, unknown>): void {
    const set = this.listeners.get(String(event.type));
    if (set) {
      for (const fn of Array.from(set)) {
        fn(event);
      }
    }
  }
}

class FakeElement extends FakeNode {
  tagName: string;
  attributes = new Map<string, string>();
  style: Record<string, string> = {};
  textContent = '';

  constructor(tagName: string) {
    super();
    this.tagName = tagName.toUpperCase();
  }

  setAttribute(name: string, val: string): void {
    this.attributes.set(name, String(val));
  }

  getAttribute(name: string): string | null {
    return this.attributes.get(name) ?? null;
  }

  removeAttribute(name: string): void {
    this.attributes.delete(name);
  }

  getBoundingClientRect(): { left: number; top: number; width: number; height: number } {
    return { left: 0, top: 0, width: 220, height: 200 };
  }

  findAll(predicate: (el: FakeElement) => boolean): FakeElement[] {
    const results: FakeElement[] = [];
    const visit = (node: FakeNode): void => {
      if (node instanceof FakeElement && predicate(node)) {
        results.push(node);
      }
      for (const child of node.childNodes) {
        visit(child);
      }
    };
    visit(this);
    return results;
  }

  collectText(): string {
    const parts: string[] = [];
    const visit = (node: FakeNode): void => {
      if (node instanceof FakeElement && node.textContent) {
        parts.push(node.textContent);
      } else if (
        'nodeValue' in node &&
        typeof (node as { nodeValue?: unknown }).nodeValue === 'string'
      ) {
        parts.push((node as { nodeValue: string }).nodeValue);
      }
      for (const child of node.childNodes) {
        visit(child);
      }
    };
    visit(this);
    return parts.join('');
  }
}

function patchDomRender(comp: Record<string, unknown>, relPath: string): () => void {
  const fullPath = path.resolve(process.cwd(), relPath);
  const { descriptor } = parse(fs.readFileSync(fullPath, 'utf8'));
  const script = compileScript(descriptor, { id: relPath });
  const tpl = compileTemplate({
    source: descriptor.template!.content,
    id: relPath,
    filename: relPath,
    compilerOptions: { mode: 'function', bindingMetadata: script.bindings },
  });
  const renderFn = new Function('Vue', tpl.code)(vueRuntime) as unknown;
  const prevRender = comp.render;
  const prevSsrRender = comp.ssrRender;
  comp.render = renderFn;
  comp.ssrRender = undefined;
  return () => {
    comp.render = prevRender;
    comp.ssrRender = prevSsrRender;
  };
}

describe('QnContextMenu (M5-05c)', () => {
  it('renders submenu entries on hover and emits select when clicking a submenu item', async () => {
    const restore = patchDomRender(
      QnContextMenu as unknown as Record<string, unknown>,
      'src/components/QnContextMenu.vue',
    );

    const host = new FakeElement('div');
    const { createApp } = createRenderer<FakeNode, FakeElement>({
      createElement: (tag) => new FakeElement(tag),
      createText: (text) => {
        const node = new FakeNode();
        (node as unknown as { nodeValue: string }).nodeValue = text;
        return node;
      },
      createComment: () => new FakeNode(),
      setText: (node, text) => {
        (node as unknown as { nodeValue: string }).nodeValue = text;
      },
      setElementText: (el, text) => {
        el.textContent = text;
      },
      insert: (child, parent, anchor) => parent.insertBefore(child, anchor ?? null),
      remove: (child) => child.parentNode?.removeChild(child),
      parentNode: (node) => node.parentNode as FakeElement | null,
      nextSibling: (node) => node?.nextSibling ?? null,
      querySelector: (selector) => (selector === 'body' ? host : null),
      patchProp: (el, key, prev, next) => {
        if (key.startsWith('on')) {
          const ev = key.slice(2).toLowerCase();
          if (prev) el.removeEventListener(ev, prev as Listener);
          if (next) el.addEventListener(ev, next as Listener);
        } else if (key === 'class') {
          const val = Array.isArray(next)
            ? next
                .flatMap((entry) => {
                  if (typeof entry === 'string') return [entry];
                  if (entry && typeof entry === 'object') {
                    return Object.entries(entry)
                      .filter(([, enabled]) => Boolean(enabled))
                      .map(([cls]) => cls);
                  }
                  return [];
                })
                .join(' ')
            : String(next ?? '');
          el.setAttribute('class', val);
        } else if (next == null || next === false) {
          el.removeAttribute(key);
        } else {
          el.setAttribute(key, String(next === true ? '' : next));
        }
      },
    });

    const onSelect = vi.fn();
    const onClose = vi.fn();
    const items: ContextMenuEntry[] = [
      { type: 'item', id: 'tab.close', title: '关闭标签' },
      {
        type: 'submenu',
        id: 'tab.delete',
        title: '删除',
        items: [
          {
            type: 'item',
            id: 'tab.deleteFile',
            title: '删除文件（移到废纸篓）',
            danger: true,
          },
        ],
      },
    ];

    const Wrapper = defineComponent({
      setup() {
        return () =>
          h(QnContextMenu, {
            open: true,
            x: 100,
            y: 100,
            items,
            onSelect,
            onClose,
          });
      },
    });

    const app = createApp(Wrapper);
    app.provide(ssrContextKey, { modules: new Set() });

    try {
      app.mount(host);
      await nextTick();

      const submenuTrigger = host.findAll(
        (el) =>
          el.tagName === 'BUTTON' &&
          el.collectText().includes('删除') &&
          !el.collectText().includes('删除文件（移到废纸篓）'),
      )[0];
      expect(submenuTrigger).toBeDefined();

      // 悬停子菜单所在容器或按钮以展开下一级菜单
      const wrapperEl = submenuTrigger!.parentNode as FakeElement;
      wrapperEl.dispatchEvent({ type: 'mouseenter' });
      submenuTrigger!.dispatchEvent({ type: 'mouseenter' });
      await nextTick();

      const subItemBtn = host.findAll(
        (el) => el.tagName === 'BUTTON' && el.collectText().includes('删除文件（移到废纸篓）'),
      )[0];
      expect(subItemBtn).toBeDefined();

      subItemBtn!.dispatchEvent({ type: 'click' });
      await nextTick();

      expect(onSelect).toHaveBeenCalledWith('tab.deleteFile');
      expect(onClose).toHaveBeenCalled();
    } finally {
      app.unmount();
      restore();
    }
  });
});

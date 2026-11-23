import type { SaveImageArgs, SavedImageResult } from '@/bindings/types';

export const MAX_IMAGE_SIZE_BYTES = 20 * 1024 * 1024;

const MIME_TO_EXT: Record<string, string> = {
  'image/png': 'png',
  'image/jpeg': 'jpg',
  'image/jpg': 'jpg',
  'image/webp': 'webp',
  'image/gif': 'gif',
  'image/svg+xml': 'svg',
};

const ALLOWED_IMAGE_EXTS = new Set(['png', 'jpg', 'webp', 'gif', 'svg']);

/**
 * 从文件 MIME 类型或文件名后缀提取白名单图片扩展名。
 */
export function extractImageExtFromFile(file: { name?: string; type?: string }): string | null {
  const mime = (file.type ?? '').trim().toLowerCase();
  if (mime && MIME_TO_EXT[mime]) {
    return MIME_TO_EXT[mime];
  }

  const name = (file.name ?? '').trim();
  const dotIdx = name.lastIndexOf('.');
  if (dotIdx >= 0 && dotIdx < name.length - 1) {
    const rawExt = name.slice(dotIdx + 1).toLowerCase();
    const normalized = rawExt === 'jpeg' ? 'jpg' : rawExt;
    if (ALLOWED_IMAGE_EXTS.has(normalized)) {
      return normalized;
    }
  }

  return null;
}

/**
 * 将 ArrayBuffer 分块转换为 Base64 字符串（避免大数组调用 String.fromCharCode 触发栈溢出）。
 */
export function arrayBufferToBase64(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  const chunkSize = 8192;
  let binary = '';
  for (let i = 0; i < bytes.length; i += chunkSize) {
    const slice = bytes.subarray(i, Math.min(i + chunkSize, bytes.length));
    binary += String.fromCharCode(...slice);
  }
  return btoa(binary);
}

/**
 * 生成 Markdown 图片引用片段。
 */
export function buildMarkdownImageSnippet(relPath: string, alt?: string): string {
  return `![${alt || 'image'}](${relPath})`;
}

export interface ImageInputFile {
  name?: string;
  type?: string;
  size: number;
  arrayBuffer: () => Promise<ArrayBuffer>;
}

export interface HandleImageInsertOptions {
  file: ImageInputFile;
  isLibrary: boolean;
  contextRelPath: string | null;
  saveImage: (args: SaveImageArgs) => Promise<SavedImageResult>;
  confirmExternalSave: () => Promise<boolean>;
  insertSnippet: (snippet: string) => void;
  onError: (message: string) => void;
}

export type ImageInsertOutcome = 'inserted' | 'cancelled' | 'error';

/**
 * 从剪贴板或拖拽数据中提取首张图片文件。
 */
export function extractImageFileFromDataTransfer(
  data: DataTransfer | null | undefined,
): File | null {
  if (!data) {
    return null;
  }
  if (data.files && data.files.length > 0) {
    for (let i = 0; i < data.files.length; i += 1) {
      const file = data.files.item(i);
      if (file && (file.type.startsWith('image/') || extractImageExtFromFile(file) !== null)) {
        return file;
      }
    }
  }
  if (data.items && data.items.length > 0) {
    for (let i = 0; i < data.items.length; i += 1) {
      const item = data.items[i];
      if (item && item.kind === 'file' && item.type.startsWith('image/')) {
        const file = item.getAsFile();
        if (file) {
          return file;
        }
      }
    }
  }
  return null;
}

/**
 * 统一处理图片粘贴/拖入：校验格式与体积、库外二次确认、调用后端落盘并插入 Markdown 引用。
 */
export async function handleImageInsert(
  options: HandleImageInsertOptions,
): Promise<ImageInsertOutcome> {
  const {
    file,
    isLibrary,
    contextRelPath,
    saveImage,
    confirmExternalSave,
    insertSnippet,
    onError,
  } = options;

  const ext = extractImageExtFromFile(file);
  if (!ext) {
    onError('不支持的图片格式，仅支持 PNG、JPG、WebP、GIF 与 SVG');
    return 'error';
  }

  if (file.size > MAX_IMAGE_SIZE_BYTES) {
    onError('图片大小超过 20MB 上限，无法保存');
    return 'error';
  }

  if (!isLibrary) {
    const confirmed = await confirmExternalSave();
    if (!confirmed) {
      return 'cancelled';
    }
  }

  try {
    const buffer = await file.arrayBuffer();
    const bytesBase64 = arrayBufferToBase64(buffer);
    const result = await saveImage({
      sourcePath: null,
      bytesBase64,
      ext,
      contextRelPath: isLibrary ? contextRelPath : null,
    });
    const baseName = file.name ? file.name.replace(/\.[^.]+$/, '') : '';
    const snippet = buildMarkdownImageSnippet(result.relPath, baseName || 'image');
    insertSnippet(snippet);
    return 'inserted';
  } catch (error) {
    const message =
      error instanceof Error && error.message ? error.message : '保存图片失败，请确认已配置笔记库';
    onError(message);
    return 'error';
  }
}

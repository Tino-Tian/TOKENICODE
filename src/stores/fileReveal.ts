import { FileNode } from '../lib/tauri-bridge';

/** 已知代码 / 配置 / 文档 / 图片文件扩展名——聊天里的「文件路径」识别共用这一份。 */
export const KNOWN_FILE_EXTENSIONS = new Set([
  'md', 'mdx', 'ts', 'tsx', 'js', 'jsx', 'mjs', 'cjs', 'json', 'jsonl',
  'toml', 'yaml', 'yml', 'py', 'pyi', 'rs', 'go', 'html', 'htm', 'css',
  'scss', 'sass', 'less', 'vue', 'svelte', 'sh', 'bash', 'zsh', 'fish',
  'env', 'conf', 'cfg', 'ini', 'xml', 'sql', 'graphql', 'gql', 'proto',
  'lock', 'log', 'txt', 'csv', 'rb', 'php', 'java', 'kt', 'swift', 'c',
  'cpp', 'h', 'hpp', 'cs', 'r', 'lua', 'zig', 'ex', 'exs', 'erl', 'ml',
  'mli', 'tf', 'hcl', 'dockerfile', 'makefile', 'png', 'jpg', 'jpeg',
  'gif', 'svg', 'webp', 'ico', 'wasm', 'map', 'pdf', 'doc', 'docx',
]);

/**
 * 判断一段反引号内的文本是不是路径，是文件还是文件夹。
 *
 * 规则放宽以支持中文路径（旧规则只认英文前缀路径，中文一律漏掉）：
 * - 无空格 + 末尾是已知扩展名 → 'file'（含中文路径 / 相对路径 / 裸文件名）
 * - 无空格 + 以 / 结尾 → 'folder'
 * - 其余（含空格的全路径、普通行内代码如 useState / Math.PI）→ null
 *
 * 「无空格」是关键护栏：避免把普通句子或带空格的全路径误判成可点路径。
 */
export function classifyPathToken(text: string): 'file' | 'folder' | null {
  const trimmed = text.trim();
  if (trimmed.length <= 1 || /\s/.test(trimmed)) return null;
  const ext = trimmed.split('.').pop()?.toLowerCase() ?? '';
  if (KNOWN_FILE_EXTENSIONS.has(ext)) return 'file';
  if (trimmed.endsWith('/')) return 'folder';
  return null;
}

/** 把路径文本解析成绝对路径（相对路径拼到 base 下），并去掉末尾斜杠，便于和文件树节点匹配。 */
export function resolvePathToken(text: string, base: string): string {
  const cleaned = text.trim().replace(/\/+$/, '');
  if (cleaned.startsWith('/') || /^[a-zA-Z]:[/\\]/.test(cleaned)) return cleaned;
  return base ? `${base.replace(/\/$/, '')}/${cleaned}` : cleaned;
}

/** 去掉路径末尾的斜杠（一个或多个）；根 '/' 本身保留。 */
export function normalizePath(p: string): string {
  if (p === '/') return p;
  return p.replace(/\/+$/, '');
}

/**
 * 算出「要让 targetPath 在文件树里可见、需要展开哪些文件夹」。
 *
 * 纯函数：返回 targetPath 在 rootPath 之下的所有祖先目录；若 targetIsDir，
 * 连 targetPath 自身也算进去（点文件夹＝把它打开）。抽成纯函数是为了能在
 * node 环境直接单测——中文 / 空格 / 末尾斜杠 / 不在根目录下，这些边界都在这里收口。
 *
 * - targetPath 不在 rootPath 之下 → 空数组（无法定位）。
 * - targetPath 就是 rootPath → 空数组（根本身不需要展开）。
 * - 顶层文件（直接在 root 下）→ 空数组（没有需要展开的祖先）。
 * - 始终返回新数组，不改动入参。
 */
export function computeRevealExpansions(
  targetPath: string,
  rootPath: string,
  targetIsDir: boolean,
): string[] {
  const target = normalizePath(targetPath);
  const root = normalizePath(rootPath);
  if (!root || !target) return [];
  if (target === root) return [];
  if (!target.startsWith(root + '/')) return [];

  const rel = target.slice(root.length + 1);
  const parts = rel.split('/').filter(Boolean);
  const result: string[] = [];
  let cur = root;
  // 祖先目录＝除最后一段外的每一级
  for (let i = 0; i < parts.length - 1; i++) {
    cur = `${cur}/${parts[i]}`;
    result.push(cur);
  }
  // 目标本身是文件夹 → 一并展开（"打开"它）
  if (targetIsDir) result.push(target);
  return result;
}

/**
 * 在文件树里按绝对路径查找节点（用来判断目标是文件还是文件夹）。
 * 树只加载到有限深度，找不到时返回 null（调用方回退到「末尾斜杠」判断）。
 */
export function findNodeByPath(tree: FileNode[], path: string): FileNode | null {
  const target = normalizePath(path);
  for (const node of tree) {
    const nodePath = normalizePath(node.path);
    if (nodePath === target) return node;
    if (node.is_dir && node.children && target.startsWith(nodePath + '/')) {
      const found = findNodeByPath(node.children, target);
      if (found) return found;
    }
  }
  return null;
}

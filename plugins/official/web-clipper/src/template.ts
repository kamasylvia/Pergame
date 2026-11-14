/**
 * Minimal template engine for note templates.
 *
 * Ported from ReadItLater (MIT, Dominik Pieper / SiYuan port) and reduced to
 * what the clipper needs: `{{ path.to.value | filter(arg) }}`, `{% for x in
 * list %}…{% endfor %}`, and the filters documented in the plugin docs.
 */

const VARIABLE_RE = /\{\{(.*?)\}\}/g;
const FOR_RE = /\{%\s*for\s+(\w+)\s+in\s+([\w.]+)\s*%\}([\s\S]*?)\{%\s*endfor\s*%\}/g;

type Data = Record<string, unknown>;

function lookup(data: Data, path: string): unknown {
  return path.split('.').reduce<unknown>((value, key) => {
    if (value && typeof value === 'object' && key in (value as Data)) {
      return (value as Data)[key];
    }
    return undefined;
  }, data);
}

function lexify(value: number): string {
  const abs = Math.abs(value);
  if (abs >= 1_000_000_000) return `${(value / 1_000_000_000).toFixed(1)}B`;
  if (abs >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
  if (abs >= 1_000) return `${(value / 1_000).toFixed(1)}K`;
  return String(value);
}

interface FilterCall {
  name: string;
  args: string[];
}

/** Split `a.b | filter("x") | other` into a path plus filter calls. */
function parseExpression(expression: string): { path: string; filters: FilterCall[] } {
  const [rawPath, ...rawFilters] = expression.split('|').map(part => part.trim());
  const filters = rawFilters.map(part => {
    const match = part.match(/^(\w+)(?:\((.*)\))?$/);
    if (!match) return { name: part, args: [] };
    const args = (match[2] ?? '')
      .split(',')
      .map(arg => arg.trim())
      .filter(Boolean)
      .map(arg => arg.replace(/^["']|["']$/g, ''));
    return { name: match[1]!, args };
  });
  return { path: rawPath ?? '', filters };
}

function applyFilter(name: string, value: unknown, args: string[]): unknown {
  const text = value == null ? '' : String(value);
  switch (name) {
    case 'blockquote':
      return text.split('\n').map(line => `> ${line}`).join('\n');
    case 'capitalize':
      return text.charAt(0).toUpperCase() + text.slice(1);
    case 'upper':
      return text.toUpperCase();
    case 'lower':
      return text.toLowerCase();
    case 'replace':
      return text.split(args[0] ?? '').join(args[1] ?? '');
    case 'join':
      return Array.isArray(value) ? value.join(args[0] ?? ', ') : text;
    case 'striptags':
      return text.replace(/<[^>]*>/g, '');
    case 'map':
      return Array.isArray(value)
        ? value.map(item => (item && typeof item === 'object' ? (item as Data)[args[0] ?? ''] : item))
        : value;
    case 'numberLexify':
      return typeof value === 'number' ? lexify(value) : text;
    case 'default':
      return text || (args[0] ?? '');
    default:
      return value;
  }
}

/** Render one variable expression against `data`. */
function renderExpression(expression: string, data: Data): string {
  const { path, filters } = parseExpression(expression);
  let value = lookup(data, path);
  for (const filter of filters) value = applyFilter(filter.name, value, filter.args);
  return value == null ? '' : String(value);
}

/** Render one template against `data`. Unknown variables become empty. */
export function renderTemplate(template: string, data: Data): string {
  const withLoops = template.replace(FOR_RE, (_match, itemName: string, listPath: string, body: string) => {
    const list = lookup(data, listPath);
    if (!Array.isArray(list)) return '';
    return list
      .map(item => {
        const scoped: Data = { ...data, [itemName]: item };
        return body.replace(VARIABLE_RE, (_m, expression: string) => renderExpression(expression, scoped));
      })
      .join('');
  });

  return withLoops.replace(VARIABLE_RE, (_match, expression: string) => renderExpression(expression, data));
}

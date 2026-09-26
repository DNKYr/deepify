// Independent Node reference for the shared Rust/extension fixture matrix.
import { isIP } from 'node:net';

function localAddress(host) {
  if (host === 'localhost' || host === '::1') return true;
  if (isIP(host) === 4) {
    const [a, b] = host.split('.').map(Number);
    return a === 127 || a === 10 || (a === 192 && b === 168) ||
      (a === 169 && b === 254) || (a === 172 && b >= 16 && b <= 31);
  }
  if (isIP(host) !== 6) return false;
  const mapped = host.match(/^::ffff:([0-9a-f]+):([0-9a-f]+)$/i);
  if (mapped) {
    const a = Number.parseInt(mapped[1], 16), b = Number.parseInt(mapped[2], 16);
    return localAddress(`${a >> 8}.${a & 255}.${b >> 8}.${b & 255}`);
  }
  return /^(fc|fd|fe[89ab])/i.test(host);
}

export function allowed(value, rules) {
  let parsed;
  try { parsed = new URL(value); } catch { return !/^http/i.test(value); }
  if (!['http:', 'https:'].includes(parsed.protocol)) return true;
  const host = parsed.hostname.toLowerCase().replace(/^\[|\]$/g, '');
  if (localAddress(host)) return true;
  return rules.some(rule => {
    let parsedRule;
    try { parsedRule = new URL(/^https?:\/\//.test(rule) ? rule : `http://${rule}`); }
    catch { return false; }
    const ruleHost = parsedRule.hostname.toLowerCase().replace(/^\[|\]$/g, '').replace(/\.$/, '');
    return (host === ruleHost || host.endsWith(`.${ruleHost}`)) && parsed.pathname.startsWith(parsedRule.pathname);
  });
}

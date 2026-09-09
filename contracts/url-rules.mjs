export function allowed(url, rules) {
  let parsed;
  try {
    parsed = new URL(url);
  } catch {
    // Malformed input cannot be a navigable public HTTP(S) destination.
    return true;
  }
  if (!['http:', 'https:'].includes(parsed.protocol)) return true;
  const host = parsed.hostname.toLowerCase().replace(/^\[/, '').replace(/\]$/, '');
  const local172 = host.startsWith('172.') && Number(host.split('.')[1]) >= 16 && Number(host.split('.')[1]) <= 31;
  const localV6 = host === '::1' || host.startsWith('fc') || host.startsWith('fd') || host.startsWith('fe8') || host.startsWith('fe9') || host.startsWith('fea') || host.startsWith('feb');
  const mappedV4 = host.match(/^::ffff:([0-9a-f]{1,4}):([0-9a-f]{1,4})$/i);
  const mappedLocal = mappedV4 && (() => {
    const first = Number.parseInt(mappedV4[1], 16);
    const second = Number.parseInt(mappedV4[2], 16);
    const mapped = `${first >> 8}.${first & 255}.${second >> 8}.${second & 255}`;
    return mapped.startsWith('127.') || mapped.startsWith('192.168.') || mapped.startsWith('169.254.') || mapped.startsWith('10.') || (mapped.startsWith('172.') && Number(mapped.split('.')[1]) >= 16 && Number(mapped.split('.')[1]) <= 31);
  })();
  if (host === 'localhost' || host.startsWith('127.') || localV6 || mappedLocal || host.startsWith('192.168.') || host.startsWith('169.254.') || host.startsWith('10.') || local172) return true;
  return rules.some(rule => {
    const normalized = rule.toLowerCase().replace(/^http:\/\//, '').replace(/^https:\/\//, '');
    const [rawHost, ...rest] = normalized.split('/');
    const ruleHost = rawHost.replace(/^\[/, '').replace(/\]$/, '').replace(/:\d+$/, '');
    const path = `/${rest.join('/')}`.replace(/\/$/, '') || '/';
    return (host === ruleHost || host.endsWith(`.${ruleHost}`)) && parsed.pathname.startsWith(path);
  });
}

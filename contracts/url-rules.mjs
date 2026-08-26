export function allowed(url, rules) {
  const parsed = new URL(url);
  if (!['http:', 'https:'].includes(parsed.protocol)) return true;
  const host = parsed.hostname.toLowerCase().replace(/^\[/, '').replace(/\]$/, '');
  const local172 = host.startsWith('172.') && Number(host.split('.')[1]) >= 16 && Number(host.split('.')[1]) <= 31;
  if (host === 'localhost' || host === '127.0.0.1' || host === '::1' || host.startsWith('192.168.') || host.startsWith('10.') || local172) return true;
  return rules.some(rule => { const normalized = rule.toLowerCase().replace(/^http:\/\//, '').replace(/^https:\/\//, ''); const [ruleHost, ...rest] = normalized.split('/'); const path = `/${rest.join('/')}`.replace(/\/$/, '') || '/'; return (host === ruleHost || host.endsWith(`.${ruleHost}`)) && parsed.pathname.startsWith(path); });
}

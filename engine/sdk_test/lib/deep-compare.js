export function deepCompare(algolia, flapjack, opts = {}) {
  const ignore = new Set(opts.ignore || ['processingTimeMS', 'taskID', 'cursor', 'serverTimeMS', 'processingTimingsMS', 'params', 'serverTimeMS']);
  const diffs = [];
  
  function walk(a, f, path = '') {
    if (ignore.has(path)) return;
    
    const typeA = typeof a;
    const typeF = typeof f;
    
    if (typeA !== typeF) {
      diffs.push({ path, issue: 'type_mismatch', algolia: typeA, flapjack: typeF });
      return;
    }
    
    if (a === null || f === null) {
      if (a !== f) diffs.push({ path, issue: 'null_mismatch', algolia: a, flapjack: f });
      return;
    }
    
    if (Array.isArray(a)) {
      if (!Array.isArray(f)) {
        diffs.push({ path, issue: 'type_mismatch', algolia: 'array', flapjack: typeF });
        return;
      }
      if (a.length !== f.length) {
        diffs.push({ path, issue: 'array_length', algolia: a.length, flapjack: f.length });
      }
      const maxLen = Math.max(a.length, f.length);
      for (let i = 0; i < maxLen; i++) {
        walk(a[i], f[i], `${path}[${i}]`);
      }
      return;
    }
    
    if (typeA === 'object') {
      const keysA = Object.keys(a).sort();
      const keysF = Object.keys(f).sort();
      
      const missing = keysA.filter(k => !keysF.includes(k));
      const extra = keysF.filter(k => !keysA.includes(k));
      
      if (missing.length) diffs.push({ path, issue: 'missing_keys', keys: missing });
      if (extra.length) diffs.push({ path, issue: 'extra_keys', keys: extra });
      
      const common = keysA.filter(k => keysF.includes(k));
      for (const k of common) {
        walk(a[k], f[k], path ? `${path}.${k}` : k);
      }
      return;
    }
    
    if (a !== f) {
      diffs.push({ 
        path, 
        issue: 'value_mismatch',
        algolia: String(a).substring(0, 100),
        flapjack: String(f).substring(0, 100)
      });
    }
  }
  
  walk(algolia, flapjack);
  return diffs;
}

export function formatDiffs(diffs, verbose = false) {
  if (diffs.length === 0) return '✅ MATCH';
  
  if (!verbose) {
    console.log('[FIRST 3 DIFFS]', JSON.stringify(diffs.slice(0, 3), null, 2));
    return `${diffs.length} divergence(s)`;
  }
  
  const MAX_SHOWN = 5;
  const lines = ['Details:', ''];
  const shown = diffs.slice(0, MAX_SHOWN);
  
  for (const d of shown) {
    lines.push(`  ${d.path || '(root)'}`);
    lines.push(`    Issue: ${d.issue}`);
    
    if (d.issue === 'missing_keys') {
      lines.push(`    Algolia has: ${d.keys.join(', ')}`);
      lines.push(`    Flapjack missing these`);
    } else if (d.issue === 'extra_keys') {
      lines.push(`    Flapjack has: ${d.keys.join(', ')}`);
      lines.push(`    Algolia doesn't have these`);
    } else if (d.keys) {
      lines.push(`    Keys: ${d.keys.join(', ')}`);
    }
    
    if (d.algolia !== undefined) lines.push(`    Algolia:  ${d.algolia}`);
    if (d.flapjack !== undefined) lines.push(`    Flapjack: ${d.flapjack}`);
    lines.push('');
  }
  
  if (diffs.length > MAX_SHOWN) {
    lines.push(`  ... and ${diffs.length - MAX_SHOWN} more divergences`);
    lines.push('');
  }
  
  return lines.join('\n');
}
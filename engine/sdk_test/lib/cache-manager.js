import fs from 'fs/promises';
import path from 'path';
import crypto from 'crypto';

const CACHE_DIR = '.cache';

export class CacheManager {
  constructor() {
    this.hashes = new Map();
  }
  
  async get(suite, caseNum) {
    const hash = await this._computeHash(suite);
    const cachePath = path.join(CACHE_DIR, suite.name, hash, `case_${caseNum}_algolia.json`);
    
    try {
      const data = await fs.readFile(cachePath, 'utf-8');
      return JSON.parse(data);
    } catch {
      return null;
    }
  }
  
  async set(suite, caseNum, data) {
    const hash = await this._computeHash(suite);
    const hashDir = path.join(CACHE_DIR, suite.name, hash);
    const cachePath = path.join(hashDir, `case_${caseNum}_algolia.json`);
    
    await fs.mkdir(hashDir, { recursive: true });
    await fs.writeFile(cachePath, JSON.stringify(data, null, 2));
  }
  
  async clear() {
    await fs.rm(CACHE_DIR, { recursive: true, force: true });
    console.log('Cache cleared');
  }
  
  async _computeHash(suite) {
    if (this.hashes.has(suite.name)) {
      return this.hashes.get(suite.name);
    }
    
    const testPath = path.join('suites', `${suite.name}.test.js`);
    const fixturePath = path.join('fixtures', 'products.json');
    
    const testCode = await fs.readFile(testPath, 'utf-8');
    const fixtures = await fs.readFile(fixturePath, 'utf-8');
    
    const hash = crypto.createHash('sha256')
      .update(testCode)
      .update(fixtures)
      .digest('hex')
      .slice(0, 8);
    
    this.hashes.set(suite.name, hash);
    return hash;
  }
}
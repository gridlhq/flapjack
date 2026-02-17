const algoliasearch = require('algoliasearch');
const dotenv = require('dotenv');
const path = require('path');

dotenv.config({ path: path.join(__dirname, '..', '.secret', '.env.secret') });
const FLAPJACK_ADMIN_KEY = process.env.FLAPJACK_ADMIN_KEY || 'fj_test_admin_key_for_local_dev';

const client = algoliasearch('flapjack', FLAPJACK_ADMIN_KEY);
client.hosts = {
  read: [{url: 'localhost:7700', protocol: 'http'}],
  write: [{url: 'localhost:7700', protocol: 'http'}]
};

const index = client.initIndex('products');

(async () => {
  try {
    await index.setSettings({attributesForFaceting: ['category']});
    console.log('✓ Settings');
    
    const upload = await index.saveObjects([
      {objectID: '1', name: 'Test Product', category: 'electronics'}
    ]);
    console.log('✓ Upload taskID:', upload.taskID);
    
    await index.waitTask(upload.taskID);
    console.log('✓ Task completed');
    
    const results = await index.search('test');
    console.log('✓ Search hits:', results.nbHits);
  } catch (e) {
    console.error('✗', e.message);
  }
})();

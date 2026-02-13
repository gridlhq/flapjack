const algoliasearch = require('algoliasearch');

const client = algoliasearch('test-app', 'test-key');
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

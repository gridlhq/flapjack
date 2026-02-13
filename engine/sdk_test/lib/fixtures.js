
import fs from 'fs/promises';
import path from 'path';

export async function generateFixtures() {
  const products = [];
  
  for (let skip = 0; skip < 50; skip += 30) {
    const response = await global.fetch(`https://dummyjson.com/products?limit=30&skip=${skip}`);
    const data = await response.json();
    products.push(...data.products.map(p => ({
      objectID: String(p.id),
      name: p.title,
      brand: p.brand,
      price: p.price,
      category: p.category,
      description: p.description,
      rating: p.rating,
      stock: p.stock,
      tags: p.tags
    })));
  }
  
  const fixturePath = path.join(process.cwd(), 'fixtures', 'products.json');
  await fs.mkdir(path.dirname(fixturePath), { recursive: true });
  await fs.writeFile(fixturePath, JSON.stringify(products.slice(0, 50), null, 2));
  
  console.log(`Generated 50 products → ${fixturePath}`);
  return products.slice(0, 50);
}

export async function loadFixtures() {
  const fixturePath = path.join(process.cwd(), 'fixtures', 'products.json');
  
  try {
    const data = await fs.readFile(fixturePath, 'utf-8');
    return JSON.parse(data);
  } catch (e) {
    console.log('Fixtures not found, generating...');
    return generateFixtures();
  }
}
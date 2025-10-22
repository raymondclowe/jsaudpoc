const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch({ headless: false });
  const page = await browser.newPage();
  await page.goto('https://raymondclowe.github.io/jsaudpoc/');

  // Wait for page and UI
  await page.waitForSelector('#record');
  await page.waitForTimeout(1000);

  // Enter a dummy Replicate API key (replace with a real one for actual test)
  await page.fill('#apiKey', 'DUMMY_KEY');
  await page.click('button:has-text("Save Key")');
  await page.waitForTimeout(500);

  // Click record
  await page.click('#record');
  await page.waitForTimeout(2000); // simulate 2s recording

  // Click stop
  await page.click('#stop');
  await page.waitForTimeout(2000);

  // Check status text
  const status = await page.textContent('#recordStatus');
  console.log('Status after stop:', status);

  await browser.close();
})();

import {test,expect} from '@playwright/test';

test('theme follows the system, remembers an override, and resumes system updates',async({page})=>{
  await page.emulateMedia({colorScheme:'dark'});
  await page.goto('/');
  const theme=page.getByRole('group',{name:'Color theme'});
  await expect(theme.getByRole('button',{name:'System theme'})).toHaveAttribute('aria-pressed','true');
  await expect(page.locator('html')).toHaveAttribute('data-theme','dark');
  await expect(theme.getByRole('button',{name:'Light theme'})).toHaveAttribute('aria-pressed','false');
  await theme.getByRole('button',{name:'Light theme'}).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme','light');
  await page.reload();
  await expect(theme.getByRole('button',{name:'Light theme'})).toHaveAttribute('aria-pressed','true');
  await expect(page.locator('html')).toHaveAttribute('data-theme','light');
  await theme.getByRole('button',{name:'Dark theme'}).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme','dark');
  await theme.getByRole('button',{name:'System theme'}).click();
  await page.emulateMedia({colorScheme:'light'});
  await expect(page.locator('html')).toHaveAttribute('data-theme','light');
});

test('theme remains usable when storage is unavailable',async({page})=>{
  await page.addInitScript(()=>{
    Object.defineProperty(window,'localStorage',{get(){throw new Error('Storage unavailable');}});
  });
  await page.goto('/');
  await page.getByRole('button',{name:'Dark theme'}).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme','dark');
  await expect(page.getByRole('heading',{name:'A clearer view of your network.'})).toBeVisible();
});

test('dark palette covers the loaded workspace and fits a narrow screen',async({page})=>{
  await page.goto('/');
  await page.getByRole('button',{name:'Dark theme'}).click();
  await page.getByLabel('Open RAW file').setInputFiles('MemphisCase2026_Mar7.RAW');
  await expect(page.locator('#network-canvas canvas').first()).toBeVisible();
  await expect(page.locator('.topbar')).toHaveCSS('background-color','rgb(24, 35, 45)');
  await expect(page.locator('.graph-wrap')).toHaveCSS('background-color','rgb(17, 28, 37)');
  await page.screenshot({path:'test-results/dark-mode-desktop.png',fullPage:true});
  await page.setViewportSize({width:320,height:800});
  await expect(page.getByRole('group',{name:'Color theme'})).toBeVisible();
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBe(true);
  await page.screenshot({path:'test-results/dark-mode-mobile.png',fullPage:true});
});

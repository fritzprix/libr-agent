const sleep = ms => new Promise(r => setTimeout(r, ms));
async function test() {
  console.log('Testing loops vs Promise.all with 10 elements...');
  const items = Array.from({length: 10}, (_, i) => i);

  const start = Date.now();
  for (const item of items) {
    await sleep(20);
  }
  console.log('for...of loop took:', Date.now() - start);

  const start2 = Date.now();
  await Promise.all(items.map(async item => {
    await sleep(20);
  }));
  console.log('Promise.all took:', Date.now() - start2);
}
test();

const sleep = ms => new Promise(r => setTimeout(r, ms));
async function test() {
  const start = Date.now();
  await Promise.all([sleep(100), sleep(100), sleep(100)]);
  console.log('Promise.all took:', Date.now() - start);

  const start2 = Date.now();
  await sleep(100);
  await sleep(100);
  await sleep(100);
  console.log('Sequential took:', Date.now() - start2);
}
test();

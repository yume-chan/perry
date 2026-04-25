function a() {
  let count = 0;
  const inc = () => count++;
  const get = () => count;

  inc();
  inc();

  return get();
}

console.log(a());

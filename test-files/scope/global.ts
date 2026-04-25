const f = (() => {
  let count = 0;
  return () => ++count;
})();

console.log(f());

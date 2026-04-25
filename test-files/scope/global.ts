const f = (() => {
  let count = 0;
  return () => ++count;
})();

console.log(f(), 'should be 1');
console.log(f(), 'should be 2');

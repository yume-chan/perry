function a(fn: () => number) {
  return fn();
}

console.log(a((b = 42) => b), 'should be 42');

function b(b = 42) {
  return b;
}

console.log(a(b), 'should be 42');

console.log(b(), 'should be 42');

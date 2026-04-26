function a(fn: (a: number) => number) {
  return fn(42);
}

console.log(a((a, b = 42) => a + b), 'should be 84');

function b(a: number, b = 42) {
  return a + b;
}

console.log(a(b), 'should be 84');

console.log(b(42), 'should be 84');

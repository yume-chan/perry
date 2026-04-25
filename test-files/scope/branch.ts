function createClosure(a: number, b: number) {
  if (a > 5) {
    let c = b + 1;
    return () => a + c;
  } else {
    return () => a + 1;
  }
}

const adder = createClosure(6, 3);
console.log(adder(), 'should be 10');

function outer(x: number) {
  console.log("outer called with", x);
  const cl = () => {
    console.log("closure called, x from scope should be", x);
    return x + 1;
  };
  console.log("returning closure");
  return cl;
}

const fn = outer(5);
console.log("calling closure");
const result = fn();
console.log("closure returned", result);

function outer(x: number) {
  return () => {
    console.log("inside closure");
    return x + 1;
  };
}

const fn = outer(5);
console.log("fn is", typeof fn);
console.log("result is", fn());

function f() {
  if (true) {
    let v: string | undefined;
    const g = () => {
      v = 'Hello';
    }
    console.log(v, 'should be undefined');
    g();
    console.log(v, 'should be Hello');
  }
}

f();

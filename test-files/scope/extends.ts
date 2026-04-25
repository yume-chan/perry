class A {
  fn(a: string) {
    console.log(a)
  }
}

class B extends A {
  fn(a = 'Hello') {
    console.log(a, 'should be Hello');
  }
}

new B().fn()


function fn(a = 'Hello') {
  console.log(a, 'should be Hello');
}

fn()

class C {
  fn(a = 'Hello') {
    console.log(a, 'should be Hello');
  }
}

const c = new C();
c.fn()

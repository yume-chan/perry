function a(v: string) {
  var v: string;

  console.log(v);
}

function b(v: string) {
  var v: string = 'World';

  console.log(v);
}

a("Hello");
console.log('should be Hello');

b("Hello");
console.log('should be World');

mkdir -p bin
mkdir -p ll

RUSTFLAGS=-Awarnings cargo build

echo ==============
echo branch
PERRY_SAVE_LL=./ll ../../target/debug/perry compile --no-auto-optimize branch.ts -o bin/native
./bin/native

echo ==============
echo closure-wrong-type
PERRY_SAVE_LL=./ll ../../target/debug/perry compile --no-auto-optimize closure-wrong-type.ts -o bin/native
./bin/native

echo ==============
echo complex
PERRY_SAVE_LL=./ll ../../target/debug/perry compile --no-auto-optimize complex.ts -o bin/native
./bin/native

echo ==============
echo extends
PERRY_SAVE_LL=./ll ../../target/debug/perry compile --no-auto-optimize extends.ts -o bin/native
./bin/native

echo ==============
echo fake-var
PERRY_SAVE_LL=./ll ../../target/debug/perry compile --no-auto-optimize fake-var.ts -o bin/native
./bin/native

echo ==============
echo global
PERRY_SAVE_LL=./ll ../../target/debug/perry compile --no-auto-optimize global.ts -o bin/native
./bin/native

echo ==============
echo not-closure
PERRY_SAVE_LL=./ll ../../target/debug/perry compile --no-auto-optimize not-closure.ts -o bin/native
./bin/native

echo ==============
echo recursive
PERRY_SAVE_LL=./ll ../../target/debug/perry compile --no-auto-optimize recursive.ts -o bin/native
./bin/native

echo ==============
echo shared
PERRY_SAVE_LL=./ll ../../target/debug/perry compile --no-auto-optimize shared.ts -o bin/native
./bin/native

import { something } from './mod';

interface MyInterface {
    name: string;
}

class MyClass {
    constructor(private name: string) {}
    
    getName(): string {
        return this.name;
    }
}

function decorated(): void {}

async function asyncFunc(): Promise<void> {}

const arrowFunc = () => {};

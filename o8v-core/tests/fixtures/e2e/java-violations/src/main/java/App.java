import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;

public class App {
    public static void main(String[] args) {
        // javac: unused import HashMap

        // javac: unused variable 'list'
        List<String> list = new ArrayList<>();

        // javac: unused variable 'x'
        int x = 42;

        System.out.println("Hello, World!");
    }
}

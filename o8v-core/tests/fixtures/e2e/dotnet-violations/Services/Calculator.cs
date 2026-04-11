namespace App.Services;

public class Calculator
{
    public int Add(int a, int b)
    {
        try
        {
            return a + b;
        }
        catch (System.Exception e)
        {
            return 0;
        }
    }

    public string? GetLabel() => null;

    public int Process()
    {
        string? label = GetLabel();
        return label.Length;
    }
}

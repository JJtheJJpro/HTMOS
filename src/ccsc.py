import re;

def camel_to_snake(name):
    """Convert CamelCase to snake_case."""
    return re.sub(r'(?<!^)(?=[A-Z])', '_', name).upper()

def process_file(input_file, output_file):
    """Read the input file and write snake_case words to the output file."""
    with open(input_file, 'r') as infile, open(output_file, 'w') as outfile:
        for line in infile:
            words = line.strip().split()
            converted_words = [camel_to_snake(word) for word in words]
            outfile.write(" ".join(converted_words) + "\n")

if __name__ == "__main__":
    input_file = 'C:\\Users\\jjthe\\Desktop\\Visual-Code-Programs\\uefi-rust\\src\\list2.txt'  # Replace with your input file path
    output_file = 'C:\\Users\\jjthe\\Desktop\\Visual-Code-Programs\\uefi-rust\\src\\list2o.txt'  # Replace with your output file path
    process_file(input_file, output_file)
    print(f"Converted words written to {output_file}")
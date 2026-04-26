class Utils
  def format_name(name)
    name.upcase
  end

  def parse_config
    data = { key: 'value' }
    unused_hash = { a: 1, b: 2 }
    data
  end

  def calculate_total(items)
    total = 0
    items.each do |item|
      total = total + item[:amount]
    end
    total
  end
end

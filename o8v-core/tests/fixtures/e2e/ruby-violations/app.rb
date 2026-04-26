class Application
  def initialize
    unused_var = 'test'
    @name = 'john'
    @email = 'john@example.com'
  end

  def process_user
    user_data = get_user_data
    puts "Processing #{@name}"
    validate_email(@email)
  end

  def get_user_data
    {
      name: @name,
      email: @email
    }
  end

  def validate_email(email)
    email.include?('@')
  end
end

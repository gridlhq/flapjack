module Flapjack
  # Base exception class for errors thrown by the Flapjack
  # client library. FlapjackError will be raised by any
  # network operation if the client has not been initialized.
  # Exception ... why? A:http://www.skorks.com/2009/09/ruby-exceptions-and-exception-handling/
  #
  class FlapjackError < StandardError
  end

  # Used when hosts are unreachable
  #
  class FlapjackUnreachableHostError < FlapjackError
    attr_reader :errors

    def initialize(message, errors = [])
      errors.last&.tap do |last_error|
        message += " Last error for #{last_error[:host]}: #{last_error[:error]}"
      end

      super(message)
      @errors = errors
    end
  end

  # An exception class raised when the REST API returns an error.
  # The error code and message will be parsed out of the HTTP response,
  # which is also included in the response attribute.
  #
  class FlapjackHttpError < FlapjackError
    attr_accessor :code, :http_message

    def initialize(code, message)
      self.code = code
      self.http_message = message
      super("#{code}: #{message}")
    end
  end
end

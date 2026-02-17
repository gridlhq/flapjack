/// Abstract base class for all Flapjack exceptions.
sealed class FlapjackException implements Exception {}

/// Exception thrown when the Flapjack API returns an error.
///
/// Contains the HTTP status code and the error message returned by the API.
final class FlapjackApiException implements FlapjackException {
  /// The HTTP status code returned by the API.
  final int statusCode;

  /// The error message returned by the API.
  final dynamic error;

  /// Constructs an [FlapjackApiException] with the provided status code and error message.
  const FlapjackApiException(this.statusCode, this.error);

  @override
  String toString() {
    return 'FlapjackApiException{statusCode: $statusCode, error: $error}';
  }
}

/// Exception thrown when a request to the Flapjack API times out.
///
/// Contains the error message associated with the timeout.
final class FlapjackTimeoutException implements FlapjackException {
  /// The error message associated with the timeout.
  final dynamic error;

  /// Constructs an [FlapjackTimeoutException] with the provided error message.
  const FlapjackTimeoutException(this.error);

  @override
  String toString() {
    return 'FlapjackTimeoutException{error: $error}';
  }
}

/// Exception thrown when there is an input/output error during a request to the Flapjack API.
///
/// Contains the error message associated with the I/O error.
final class FlapjackIOException implements FlapjackException {
  /// The error message associated with the I/O error.
  final dynamic error;

  /// Constructs an [FlapjackIOException] with the provided error message.
  const FlapjackIOException(this.error);

  @override
  String toString() {
    return 'FlapjackIOException{error: $error}';
  }
}

/// Exception thrown when an error occurs during the wait strategy.
/// For example: maximum number of retry exceeded.
final class FlapjackWaitException implements FlapjackException {
  /// The error message.
  final dynamic error;

  /// Constructs an [FlapjackWaitException] with the provided error message.
  const FlapjackWaitException(this.error);

  @override
  String toString() {
    return 'FlapjackWaitException{error: $error}';
  }
}

/// Exception thrown when all hosts for the Flapjack API are unreachable.
///
/// Contains a list of the errors associated with each unreachable host.
final class UnreachableHostsException implements FlapjackException {
  /// The list of errors associated with each unreachable host.
  final List<FlapjackException> errors;
  final String message =
      "If the error persists, please visit our help center https://alg.li/support-unreachable-hosts or reach out to the Flapjack Support team: https://alg.li/support";

  /// Constructs an [UnreachableHostsException] with the provided list of errors.
  const UnreachableHostsException(this.errors);

  @override
  String toString() {
    return 'UnreachableHostsException{errors: $errors, message: $message}';
  }
}

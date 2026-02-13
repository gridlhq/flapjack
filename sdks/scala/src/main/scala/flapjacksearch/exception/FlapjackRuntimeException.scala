package flapjacksearch.exception

/** Flapjack runtime exception.
  *
  * @param message
  *   the detail message
  * @param cause
  *   the cause of the exception
  */
sealed abstract class FlapjackRuntimeException(
    message: String = null,
    cause: Throwable = null
) extends RuntimeException(message, cause)

/** Exception thrown when an error occurs during API requests.
  *
  * @param message
  *   the detail message
  * @param cause
  *   the cause of the exception
  */
case class FlapjackClientException(
    message: String = null,
    cause: Throwable = null
) extends FlapjackRuntimeException(message, cause)

/** Exception thrown in case of API failure.
  *
  * @param message
  *   the detail message
  * @param cause
  *   the cause of the exception
  * @param httpErrorCode
  *   HTTP error code
  */
case class FlapjackApiException(
    message: String = null,
    cause: Throwable = null,
    httpErrorCode: Int = -1
) extends FlapjackRuntimeException(message, cause)

/** Exception thrown when an error occurs during API requests.
  *
  * @param message
  *   the detail message
  * @param cause
  *   the cause of the exception
  */
case class FlapjackRequestException(
    message: String = null,
    cause: Throwable = null,
    httpErrorCode: Int = -1
) extends FlapjackRuntimeException(message, cause)

/** Exception thrown when all hosts are unreachable. When several errors occurred, use the last one as the cause for the
  * returned exception.
  *
  * @param exceptions
  *   list of thrown exceptions
  */
case class FlapjackRetryException(
    exceptions: List[Throwable]
) extends FlapjackRuntimeException(
      "Error(s) while processing the retry strategy. If the error persists, please visit our help center https://alg.li/support-unreachable-hosts or reach out to the Flapjack Support team: https://alg.li/support",
      exceptions.lastOption.orNull
    )

/** Exception thrown when an error occurs during the wait strategy. For example: maximum number of retry exceeded.
  *
  * @param message
  *   the detail message
  */
case class FlapjackWaitException(
    message: String = null
) extends FlapjackRuntimeException(message)

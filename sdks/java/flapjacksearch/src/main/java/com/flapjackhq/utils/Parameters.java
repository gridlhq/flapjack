package com.flapjackhq.utils;

import com.flapjackhq.exceptions.FlapjackRuntimeException;

public class Parameters {

  private Parameters() {
    // Empty.
  }

  public static void requireNonNull(Object param, String error) {
    if (param == null) {
      throw new FlapjackRuntimeException(error);
    }
  }
}

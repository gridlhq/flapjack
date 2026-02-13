package com.flapjackhq.utils;

import java.util.Iterator;
import java.util.function.BooleanSupplier;
import java.util.function.Supplier;

public class FlapjackIterableHelper {

  private FlapjackIterableHelper() {
    // Empty.
  }

  public static <T> Iterable<T> createIterable(Supplier<Iterator<T>> executeQuery, BooleanSupplier hasNext) {
    return () ->
      new Iterator<T>() {
        private boolean isFirstRequest = true;
        private Iterator<T> currentIterator = null;

        @Override
        public boolean hasNext() {
          if (isFirstRequest || (hasNext.getAsBoolean() && !currentIterator.hasNext())) {
            currentIterator = executeQuery.get();
            isFirstRequest = false;
          }
          return currentIterator != null && currentIterator.hasNext();
        }

        @Override
        public T next() {
          if (currentIterator == null || !currentIterator.hasNext()) {
            currentIterator = executeQuery.get();
            isFirstRequest = false;
          }
          return currentIterator.next();
        }
      };
  }
}

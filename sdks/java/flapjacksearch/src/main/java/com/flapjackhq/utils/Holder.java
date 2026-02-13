package com.flapjackhq.utils;

public class Holder<T> {

  public T value;

  public Holder() {
    this.value = null;
  }

  public Holder(T value) {
    this.value = value;
  }
}

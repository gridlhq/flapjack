//
//  RequestBuilder.swift
//
//
//  Created by Flapjack on 16/01/2024.
//

import Foundation

#if canImport(FoundationNetworking)
    import FoundationNetworking
#endif

// MARK: - RequestBuilder

public protocol RequestBuilder {
    init()

    func execute<T: Decodable>(urlRequest: URLRequest, timeout: TimeInterval) async throws
        -> Response<T>
}

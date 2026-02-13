# frozen_string_literal: true

require "minitest/autorun"
require "flapjack"
require "json"
require "net/http"
require "uri"

# E2E tests for Flapjack Ruby SDK against a local Flapjack server.
#
# Prerequisites:
#   - Flapjack server running on localhost:7700
#   - Environment vars: FLAPJACK_APP_ID, FLAPJACK_API_KEY (or defaults below)
#
# Run: bundle exec ruby tests/flapjack_search_e2e_test.rb
#
class FlapjackSearchE2eTest < Minitest::Test
  SERVER_HOST = ENV["FLAPJACK_SERVER"] || "localhost"
  SERVER_PORT = (ENV["FLAPJACK_PORT"] || "7700").to_i
  APP_ID = ENV["FLAPJACK_APP_ID"] || "test-app"
  API_KEY = ENV["FLAPJACK_API_KEY"] || "test-api-key"
  INDEX_NAME = "test_ruby_sdk"

  def self.client
    @client ||= begin
      hosts = [
        Flapjack::Transport::StatefulHost.new(
          SERVER_HOST,
          protocol: "http://",
          port: SERVER_PORT,
          accept: CallType::READ | CallType::WRITE
        )
      ]
      config = Flapjack::Configuration.new(APP_ID, API_KEY, hosts, "Search")
      config.connect_timeout = 5000
      config.read_timeout = 10000
      config.write_timeout = 30000
      Flapjack::SearchClient.create_with_config(config)
    end
  end

  def client
    self.class.client
  end

  # Helper: extract a field from a hit (may be in additional_properties for dynamic fields)
  def hit_field(hit, field)
    if hit.respond_to?(field.to_sym)
      hit.send(field.to_sym)
    elsif hit.respond_to?(:additional_properties) && hit.additional_properties
      hit.additional_properties[field.to_s] || hit.additional_properties[field.to_sym]
    elsif hit.is_a?(Hash)
      hit[field.to_s] || hit[field.to_sym]
    end
  end

  # Poll for task completion
  def wait_for_task(index_name, task_id, max_retries: 30)
    return if task_id.nil? || task_id <= 0
    max_retries.times do
      begin
        resp = client.get_task(index_name, task_id)
        return if resp.respond_to?(:status) && resp.status == "published"
        return if resp.is_a?(Hash) && resp["status"] == "published"
      rescue => e
        # ignore and retry
      end
      sleep 0.2
    end
  end

  # Seed test data once before all tests
  def setup
    @seeded ||= begin
      # Configure settings (include searchable(brand) for facet value search)
      settings_resp = client.set_settings(INDEX_NAME,
        Flapjack::Search::IndexSettings.new(
          searchable_attributes: ["name", "brand", "category"],
          attributes_for_faceting: ["searchable(brand)", "category", "price"]
        )
      )
      wait_for_task(INDEX_NAME, settings_resp.respond_to?(:task_id) ? settings_resp.task_id : nil)

      # Save objects
      objects = [
        { objectID: "phone1", name: "iPhone 15 Pro", brand: "Apple", category: "Phone", price: 999 },
        { objectID: "phone2", name: "Samsung Galaxy S24", brand: "Samsung", category: "Phone", price: 799 },
        { objectID: "laptop1", name: "MacBook Pro M3", brand: "Apple", category: "Laptop", price: 1999 },
        { objectID: "laptop2", name: "Google Pixel 8", brand: "Google", category: "Phone", price: 699 },
        { objectID: "laptop3", name: "Dell XPS 15", brand: "Dell", category: "Laptop", price: 1299 },
      ]

      objects.each do |obj|
        oid = obj[:objectID]
        body = obj.reject { |k, _| k == :objectID }
        resp = client.add_or_update_object(INDEX_NAME, oid, body)
        wait_for_task(INDEX_NAME, resp.respond_to?(:task_id) ? resp.task_id : nil)
      end

      sleep 0.5
      true
    end
  end

  # =========================================================================
  # List Indices
  # =========================================================================

  def test_01_list_indices
    response = client.list_indices
    items = response.respond_to?(:items) ? response.items : response["items"]
    names = items.map { |i| i.respond_to?(:name) ? i.name : i["name"] }
    assert_includes names, INDEX_NAME
  end

  # =========================================================================
  # Search Tests
  # =========================================================================

  def test_02_basic_search
    response = client.search(
      Flapjack::Search::SearchMethodParams.new(
        requests: [Flapjack::Search::SearchForHits.new(index_name: INDEX_NAME, query: "pixel")]
      )
    )
    results = response.respond_to?(:results) ? response.results : response["results"]
    assert results.length > 0
    hits = results[0].respond_to?(:hits) ? results[0].hits : results[0]["hits"]
    assert hits.length > 0
    hit_names = hits.map { |h| hit_field(h, "name") }
    found = hit_names.any? { |n| n.to_s.downcase.include?("pixel") }
    assert found, "Expected a hit containing 'pixel'"
  end

  def test_03_empty_query_returns_all
    response = client.search(
      Flapjack::Search::SearchMethodParams.new(
        requests: [Flapjack::Search::SearchForHits.new(index_name: INDEX_NAME, query: "")]
      )
    )
    results = response.respond_to?(:results) ? response.results : response["results"]
    hits = results[0].respond_to?(:hits) ? results[0].hits : results[0]["hits"]
    assert hits.length >= 5
  end

  def test_04_search_with_filters
    response = client.search(
      Flapjack::Search::SearchMethodParams.new(
        requests: [Flapjack::Search::SearchForHits.new(
          index_name: INDEX_NAME, query: "", filters: "brand:Apple"
        )]
      )
    )
    results = response.respond_to?(:results) ? response.results : response["results"]
    hits = results[0].respond_to?(:hits) ? results[0].hits : results[0]["hits"]
    assert hits.length > 0
    hits.each do |hit|
      brand = hit_field(hit, "brand")
      assert_equal "Apple", brand
    end
  end

  def test_05_search_with_facets
    response = client.search(
      Flapjack::Search::SearchMethodParams.new(
        requests: [Flapjack::Search::SearchForHits.new(
          index_name: INDEX_NAME, query: "", facets: ["brand", "category"]
        )]
      )
    )
    results = response.respond_to?(:results) ? response.results : response["results"]
    facets = results[0].respond_to?(:facets) ? results[0].facets : results[0]["facets"]
    assert facets.key?("brand") || facets.key?(:brand), "Expected 'brand' in facets"
    assert facets.key?("category") || facets.key?(:category), "Expected 'category' in facets"
  end

  def test_06_search_highlighting
    response = client.search(
      Flapjack::Search::SearchMethodParams.new(
        requests: [Flapjack::Search::SearchForHits.new(index_name: INDEX_NAME, query: "macbook")]
      )
    )
    results = response.respond_to?(:results) ? response.results : response["results"]
    hits = results[0].respond_to?(:hits) ? results[0].hits : results[0]["hits"]
    assert hits.length > 0
    hit = hits[0]
    highlight = hit.respond_to?(:_highlight_result) ? hit._highlight_result : hit_field(hit, "_highlightResult")
    refute_nil highlight, "Expected _highlightResult in hit"
  end

  def test_07_search_pagination
    response = client.search(
      Flapjack::Search::SearchMethodParams.new(
        requests: [Flapjack::Search::SearchForHits.new(
          index_name: INDEX_NAME, query: "", hits_per_page: 2
        )]
      )
    )
    results = response.respond_to?(:results) ? response.results : response["results"]
    result = results[0]
    hits = result.respond_to?(:hits) ? result.hits : result["hits"]
    assert hits.length <= 2
    nb_pages = result.respond_to?(:nb_pages) ? result.nb_pages : result["nbPages"]
    assert nb_pages > 1
  end

  def test_08_multi_index_search
    response = client.search(
      Flapjack::Search::SearchMethodParams.new(
        requests: [
          Flapjack::Search::SearchForHits.new(index_name: INDEX_NAME, query: "apple"),
          Flapjack::Search::SearchForHits.new(index_name: INDEX_NAME, query: "dell"),
        ]
      )
    )
    results = response.respond_to?(:results) ? response.results : response["results"]
    assert_equal 2, results.length
  end

  # =========================================================================
  # Object Tests
  # =========================================================================

  def test_09_get_object
    response = client.get_object(INDEX_NAME, "phone1")
    name = hit_field(response, "name")
    assert_equal "iPhone 15 Pro", name
  end

  def test_10_partial_update_object
    update_resp = client.partial_update_object(INDEX_NAME, "phone1", { price: 949 }, true)
    wait_for_task(INDEX_NAME, update_resp.respond_to?(:task_id) ? update_resp.task_id : nil)

    obj = client.get_object(INDEX_NAME, "phone1")
    price = hit_field(obj, "price")
    assert_equal 949, price

    # Restore
    restore_resp = client.partial_update_object(INDEX_NAME, "phone1", { price: 999 }, true)
    wait_for_task(INDEX_NAME, restore_resp.respond_to?(:task_id) ? restore_resp.task_id : nil)
  end

  def test_11_save_and_delete_object
    resp = client.add_or_update_object(INDEX_NAME, "temp_ruby1", { name: "Temp Product", brand: "Test", category: "Test", price: 1 })
    wait_for_task(INDEX_NAME, resp.respond_to?(:task_id) ? resp.task_id : nil)

    obj = client.get_object(INDEX_NAME, "temp_ruby1")
    name = hit_field(obj, "name")
    assert_equal "Temp Product", name

    del_resp = client.delete_object(INDEX_NAME, "temp_ruby1")
    wait_for_task(INDEX_NAME, del_resp.respond_to?(:task_id) ? del_resp.task_id : nil)
  end

  # =========================================================================
  # Settings Tests
  # =========================================================================

  def test_12_get_settings
    settings = client.get_settings(INDEX_NAME)
    attrs = settings.respond_to?(:searchable_attributes) ? settings.searchable_attributes : (settings["searchableAttributes"] || settings[:searchableAttributes])
    refute_nil attrs, "Expected searchableAttributes in settings"
  end

  def test_13_update_settings
    resp = client.set_settings(INDEX_NAME,
      Flapjack::Search::IndexSettings.new(
        searchable_attributes: ["name", "brand", "category", "price"]
      )
    )
    wait_for_task(INDEX_NAME, resp.respond_to?(:task_id) ? resp.task_id : nil)

    settings = client.get_settings(INDEX_NAME)
    attrs = settings.respond_to?(:searchable_attributes) ? settings.searchable_attributes : (settings["searchableAttributes"] || settings[:searchableAttributes])
    assert_includes attrs, "price"

    # Restore original settings
    restore_resp = client.set_settings(INDEX_NAME,
      Flapjack::Search::IndexSettings.new(
        searchable_attributes: ["name", "brand", "category"],
        attributes_for_faceting: ["searchable(brand)", "category", "price"]
      )
    )
    wait_for_task(INDEX_NAME, restore_resp.respond_to?(:task_id) ? restore_resp.task_id : nil)
  end

  # =========================================================================
  # Synonyms Tests
  # =========================================================================

  def test_14_save_and_search_synonyms
    synonym = Flapjack::Search::SynonymHit.new(
      algolia_object_id: "syn_ruby_phone",
      type: "synonym",
      synonyms: ["phone", "mobile", "cell"]
    )

    resp = client.save_synonym(INDEX_NAME, "syn_ruby_phone", synonym, true)
    wait_for_task(INDEX_NAME, resp.respond_to?(:task_id) ? resp.task_id : nil)

    search_resp = client.search_synonyms(INDEX_NAME,
      Flapjack::Search::SearchSynonymsParams.new(query: "phone")
    )
    hits = search_resp.respond_to?(:hits) ? search_resp.hits : (search_resp["hits"] || search_resp[:hits])
    refute_nil hits, "Expected hits in synonym search response"

    # Cleanup
    del_resp = client.delete_synonym(INDEX_NAME, "syn_ruby_phone", true)
    wait_for_task(INDEX_NAME, del_resp.respond_to?(:task_id) ? del_resp.task_id : nil)
  end

  # =========================================================================
  # Rules Tests
  # =========================================================================

  def test_15_save_and_search_rules
    rule = Flapjack::Search::Rule.new(
      algolia_object_id: "rule_ruby_budget",
      conditions: [Flapjack::Search::Condition.new(pattern: "budget", anchoring: "contains")],
      consequence: Flapjack::Search::Consequence.new(
        params: Flapjack::Search::ConsequenceParams.new(filters: "price < 1000")
      )
    )

    resp = client.save_rule(INDEX_NAME, "rule_ruby_budget", rule, true)
    wait_for_task(INDEX_NAME, resp.respond_to?(:task_id) ? resp.task_id : nil)

    search_resp = client.search_rules(INDEX_NAME,
      Flapjack::Search::SearchRulesParams.new(query: "budget")
    )
    hits = search_resp.respond_to?(:hits) ? search_resp.hits : (search_resp["hits"] || search_resp[:hits])
    refute_nil hits, "Expected hits in rules search response"

    # Cleanup
    del_resp = client.delete_rule(INDEX_NAME, "rule_ruby_budget", true)
    wait_for_task(INDEX_NAME, del_resp.respond_to?(:task_id) ? del_resp.task_id : nil)
  end

  # =========================================================================
  # User Agent Tests
  # =========================================================================

  def test_16_user_agent_contains_flapjack
    agent = Flapjack::UserAgent.new
    assert_match(/Flapjack for Ruby/, agent.value)
  end

  def test_17_custom_user_agent
    agent = Flapjack::UserAgent.new
    agent.add("MyApp", "1.0.0")
    assert_match(/MyApp \(1\.0\.0\)/, agent.value)
  end

  # =========================================================================
  # Browse Tests
  # =========================================================================

  def test_18_browse_with_pagination
    response = client.browse(INDEX_NAME,
      Flapjack::Search::BrowseParamsObject.new(hits_per_page: 2)
    )
    hits = response.respond_to?(:hits) ? response.hits : (response["hits"] || response[:hits])
    assert hits.length == 2
    cursor = response.respond_to?(:cursor) ? response.cursor : (response["cursor"] || response[:cursor])
    refute_nil cursor, "Expected cursor for pagination"
  end

  # =========================================================================
  # Search for Facet Values
  # =========================================================================

  def test_19_search_for_facet_values
    response = client.search_for_facet_values(INDEX_NAME, "brand",
      Flapjack::Search::SearchForFacetValuesRequest.new(facet_query: "a")
    )
    facet_hits = response.respond_to?(:facet_hits) ? response.facet_hits : (response["facetHits"] || response[:facetHits])
    refute_nil facet_hits, "Expected facetHits"
    assert facet_hits.length >= 1, "Expected at least one facet hit for prefix 'a'"
    values = facet_hits.map { |f| f.respond_to?(:value) ? f.value : (f["value"] || f[:value]) }
    assert values.include?("Apple"), "Expected 'Apple' in facet values for prefix 'a'"
  end

  # =========================================================================
  # Response Format Compliance
  # =========================================================================

  def test_20_algolia_compatible_response_fields
    response = client.search(
      Flapjack::Search::SearchMethodParams.new(
        requests: [Flapjack::Search::SearchForHits.new(index_name: INDEX_NAME, query: "macbook")]
      )
    )
    results = response.respond_to?(:results) ? response.results : response["results"]
    result = results[0]

    # Check all Algolia-compatible response fields
    hits = result.respond_to?(:hits) ? result.hits : result["hits"]
    refute_nil hits
    nb_hits = result.respond_to?(:nb_hits) ? result.nb_hits : result["nbHits"]
    refute_nil nb_hits
    page = result.respond_to?(:page) ? result.page : result["page"]
    refute_nil page
    nb_pages = result.respond_to?(:nb_pages) ? result.nb_pages : result["nbPages"]
    refute_nil nb_pages
    hits_per_page = result.respond_to?(:hits_per_page) ? result.hits_per_page : result["hitsPerPage"]
    refute_nil hits_per_page
    processing_time = result.respond_to?(:processing_time_ms) ? result.processing_time_ms : result["processingTimeMS"]
    refute_nil processing_time

    # Hits must contain _highlightResult
    hit = hits[0]
    refute_nil hit
    highlight = hit.respond_to?(:_highlight_result) ? hit._highlight_result : hit_field(hit, "_highlightResult")
    refute_nil highlight, "Expected _highlightResult in hit"
  end

  # =========================================================================
  # Cleanup
  # =========================================================================

  def test_99_cleanup
    response = client.list_indices
    items = response.respond_to?(:items) ? response.items : response["items"]
    names = items.map { |i| i.respond_to?(:name) ? i.name : i["name"] }
    assert_includes names, INDEX_NAME

    # Clean up the test index
    client.delete_index(INDEX_NAME)
  end
end

"""End-to-end tests for flapjack-search Python SDK against a local Flapjack server."""

import pytest

from flapjacksearch.search.client import SearchClientSync
from flapjacksearch.search.config import SearchConfig
from flapjacksearch.http.hosts import Host, HostsCollection, CallType
from flapjacksearch.search.models import (
    IndexSettings,
    SearchMethodParams,
    SearchForHits,
    SearchQuery,
    SynonymHit,
    SynonymType,
    Rule,
    Condition,
    Consequence,
    ConsequenceParams,
)


INDEX_NAME = "python-e2e-test"


@pytest.fixture(scope="module")
def client():
    config = SearchConfig("test-app", "test-api-key")
    config.hosts = HostsCollection(
        [Host(url="localhost:7700", scheme="http", accept=CallType.READ | CallType.WRITE)]
    )
    c = SearchClientSync.create_with_config(config=config)
    yield c
    # Cleanup
    try:
        c.delete_index(index_name=INDEX_NAME)
    except Exception:
        pass


@pytest.fixture(scope="module", autouse=True)
def seed_data(client):
    """Seed test data and wait for indexing."""
    objects = [
        {"objectID": "1", "name": "iPhone 15 Pro", "brand": "Apple", "price": 1199, "category": "phone"},
        {"objectID": "2", "name": "Galaxy S24 Ultra", "brand": "Samsung", "price": 1299, "category": "phone"},
        {"objectID": "3", "name": "Pixel 8 Pro", "brand": "Google", "price": 999, "category": "phone"},
        {"objectID": "4", "name": "MacBook Pro M3", "brand": "Apple", "price": 1999, "category": "laptop"},
        {"objectID": "5", "name": "ThinkPad X1 Carbon", "brand": "Lenovo", "price": 1499, "category": "laptop"},
    ]
    result = client.save_objects(index_name=INDEX_NAME, objects=objects)
    client.wait_for_task(index_name=INDEX_NAME, task_id=result[0].task_id)

    # Set searchable attributes
    result = client.set_settings(
        index_name=INDEX_NAME,
        index_settings=IndexSettings(
            searchable_attributes=["name", "brand", "category"],
            attributes_for_faceting=["brand", "category"],
        ),
    )
    client.wait_for_task(index_name=INDEX_NAME, task_id=result.task_id)


class TestListIndices:
    def test_list_indices(self, client):
        result = client.list_indices()
        names = [idx.name for idx in result.items]
        assert INDEX_NAME in names


class TestSearch:
    def test_basic_search(self, client):
        result = client.search(
            search_method_params=SearchMethodParams(
                requests=[SearchQuery(SearchForHits(index_name=INDEX_NAME, query="pixel"))]
            )
        )
        assert len(result.results) == 1
        r = result.results[0].actual_instance
        assert r.nb_hits >= 1
        assert r.query == "pixel"

    def test_empty_query_returns_all(self, client):
        result = client.search(
            search_method_params=SearchMethodParams(
                requests=[SearchQuery(SearchForHits(index_name=INDEX_NAME, query=""))]
            )
        )
        r = result.results[0].actual_instance
        assert r.nb_hits == 5

    def test_search_with_filters(self, client):
        result = client.search(
            search_method_params=SearchMethodParams(
                requests=[
                    SearchQuery(
                        SearchForHits(
                            index_name=INDEX_NAME,
                            query="",
                            filters="brand:Apple",
                        )
                    )
                ]
            )
        )
        r = result.results[0].actual_instance
        assert r.nb_hits == 2  # iPhone + MacBook

    def test_search_with_facets(self, client):
        result = client.search(
            search_method_params=SearchMethodParams(
                requests=[
                    SearchQuery(
                        SearchForHits(
                            index_name=INDEX_NAME,
                            query="",
                            facets=["brand", "category"],
                        )
                    )
                ]
            )
        )
        r = result.results[0].actual_instance
        assert r.facets is not None
        assert "brand" in r.facets
        assert "category" in r.facets

    def test_search_highlighting(self, client):
        result = client.search(
            search_method_params=SearchMethodParams(
                requests=[SearchQuery(SearchForHits(index_name=INDEX_NAME, query="apple"))]
            )
        )
        r = result.results[0].actual_instance
        assert r.nb_hits >= 1
        hit = r.hits[0]
        assert hit.highlight_result is not None

    def test_search_pagination(self, client):
        result = client.search(
            search_method_params=SearchMethodParams(
                requests=[
                    SearchQuery(
                        SearchForHits(
                            index_name=INDEX_NAME,
                            query="",
                            hits_per_page=2,
                            page=0,
                        )
                    )
                ]
            )
        )
        r = result.results[0].actual_instance
        assert len(r.hits) == 2
        assert r.nb_pages >= 2

    def test_multi_index_search(self, client):
        result = client.search(
            search_method_params=SearchMethodParams(
                requests=[
                    SearchQuery(SearchForHits(index_name=INDEX_NAME, query="apple")),
                    SearchQuery(SearchForHits(index_name=INDEX_NAME, query="samsung")),
                ]
            )
        )
        assert len(result.results) == 2


class TestObjects:
    def test_get_object(self, client):
        result = client.get_object(index_name=INDEX_NAME, object_id="1")
        assert result["name"] == "iPhone 15 Pro"

    def test_partial_update(self, client):
        result = client.partial_update_object(
            index_name=INDEX_NAME,
            object_id="1",
            attributes_to_update={"price": 1099},
        )
        client.wait_for_task(index_name=INDEX_NAME, task_id=result.task_id)
        obj = client.get_object(index_name=INDEX_NAME, object_id="1")
        assert obj["price"] == 1099

        # Restore original
        result = client.partial_update_object(
            index_name=INDEX_NAME,
            object_id="1",
            attributes_to_update={"price": 1199},
        )
        client.wait_for_task(index_name=INDEX_NAME, task_id=result.task_id)

    def test_save_and_delete_object(self, client):
        result = client.save_objects(
            index_name=INDEX_NAME,
            objects=[{"objectID": "temp-1", "name": "Temporary Object"}],
        )
        client.wait_for_task(index_name=INDEX_NAME, task_id=result[0].task_id)

        obj = client.get_object(index_name=INDEX_NAME, object_id="temp-1")
        assert obj["name"] == "Temporary Object"

        result = client.delete_object(index_name=INDEX_NAME, object_id="temp-1")
        client.wait_for_task(index_name=INDEX_NAME, task_id=result.task_id)


class TestSettings:
    def test_get_settings(self, client):
        settings = client.get_settings(index_name=INDEX_NAME)
        assert settings.searchable_attributes == ["name", "brand", "category"]

    def test_update_settings(self, client):
        """Test that set_settings accepts and processes settings updates."""
        result = client.set_settings(
            index_name=INDEX_NAME,
            index_settings=IndexSettings(
                searchable_attributes=["name", "brand"],
            ),
        )
        client.wait_for_task(index_name=INDEX_NAME, task_id=result.task_id)

        settings = client.get_settings(index_name=INDEX_NAME)
        assert settings.searchable_attributes == ["name", "brand"]

        # Restore
        result = client.set_settings(
            index_name=INDEX_NAME,
            index_settings=IndexSettings(
                searchable_attributes=["name", "brand", "category"],
                attributes_for_faceting=["brand", "category"],
            ),
        )
        client.wait_for_task(index_name=INDEX_NAME, task_id=result.task_id)


class TestSynonyms:
    def test_save_and_search_synonyms(self, client):
        result = client.save_synonyms(
            index_name=INDEX_NAME,
            synonym_hit=[
                SynonymHit(
                    object_id="syn-1",
                    type=SynonymType.SYNONYM,
                    synonyms=["phone", "mobile", "cell"],
                )
            ],
        )
        client.wait_for_task(index_name=INDEX_NAME, task_id=result.task_id)

        synonyms = client.search_synonyms(index_name=INDEX_NAME)
        assert len(synonyms.hits) >= 1

        # Cleanup
        result = client.delete_synonym(index_name=INDEX_NAME, object_id="syn-1")
        client.wait_for_task(index_name=INDEX_NAME, task_id=result.task_id)


class TestRules:
    def test_save_and_search_rules(self, client):
        result = client.save_rules(
            index_name=INDEX_NAME,
            rules=[
                Rule(
                    object_id="rule-1",
                    conditions=[Condition(pattern="cheap", anchoring="contains")],
                    consequence=Consequence(
                        params=ConsequenceParams(filters="price < 1000")
                    ),
                )
            ],
        )
        client.wait_for_task(index_name=INDEX_NAME, task_id=result.task_id)

        rules = client.search_rules(index_name=INDEX_NAME)
        assert len(rules.hits) >= 1

        # Cleanup
        result = client.delete_rule(index_name=INDEX_NAME, object_id="rule-1")
        client.wait_for_task(index_name=INDEX_NAME, task_id=result.task_id)


class TestUserAgent:
    def test_user_agent_contains_flapjack(self, client):
        ua = client._config.headers.get("user-agent", "")
        assert "Flapjack for Python" in ua

    def test_add_user_agent(self, client):
        client.add_user_agent("TestApp", "1.0.0")
        ua = client._config.headers.get("user-agent", "")
        assert "TestApp (1.0.0)" in ua

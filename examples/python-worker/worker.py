"""A small non-HTTP application for source-only diagram exploration.

No web framework, OpenAPI document, background scheduler, or network client is
needed. Ideomancer inspects this source without importing or running it.
"""


class Normalizer:
    def normalize(self, value: str) -> str:
        return value.strip().casefold()


class BatchService:
    def __init__(self, normalizer: Normalizer):
        self.normalizer = normalizer

    def process(self, records: list[str]) -> list[str]:
        return [self.normalizer.normalize(record) for record in records]


def run_batch(records: list[str]) -> list[str]:
    service = BatchService(Normalizer())
    return service.process(records)


def summarize(records: list[str]) -> dict[str, int]:
    normalized = run_batch(records)
    return {"records": len(normalized), "unique": len(set(normalized))}


if __name__ == "__main__":
    print(summarize([" Alpha ", "BETA", "alpha"]))

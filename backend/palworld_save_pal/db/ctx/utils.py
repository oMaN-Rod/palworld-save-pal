from contextlib import contextmanager
from sqlmodel import Session

from palworld_save_pal.db.bootstrap import engine


@contextmanager
def get_db_session():
    session = Session(engine)
    try:
        yield session
        session.commit()
    except Exception as e:
        session.rollback()
        raise e
    finally:
        session.close()

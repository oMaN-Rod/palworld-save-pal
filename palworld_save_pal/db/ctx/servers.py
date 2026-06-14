from datetime import datetime
from typing import Dict, List, Optional, Set, Any

from sqlmodel import Session, select

from palworld_save_pal.db.bootstrap import engine
from palworld_save_pal.db.models.server_models import ServerModel
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


class ServerDBService:
    @staticmethod
    def create_server(data: Dict[str, Any]) -> ServerModel:
        with Session(engine) as session:
            server = ServerModel(**data)
            session.add(server)
            session.commit()
            session.refresh(server)
            # Eagerly load all attributes before detaching
            _load_attrs(server)
            session.expunge(server)
            return server

    @staticmethod
    def get_server(server_id: int) -> Optional[ServerModel]:
        with Session(engine) as session:
            server = session.get(ServerModel, server_id)
            if server:
                _load_attrs(server)
                session.expunge(server)
            return server

    @staticmethod
    def get_server_by_container_name(name: str) -> Optional[ServerModel]:
        with Session(engine) as session:
            stmt = select(ServerModel).where(ServerModel.container_name == name)
            server = session.exec(stmt).first()
            if server:
                _load_attrs(server)
                session.expunge(server)
            return server

    @staticmethod
    def list_servers() -> List[ServerModel]:
        with Session(engine) as session:
            stmt = select(ServerModel).order_by(ServerModel.created_at)
            servers = session.exec(stmt).all()
            result = []
            for s in servers:
                _load_attrs(s)
                session.expunge(s)
                result.append(s)
            return result

    @staticmethod
    def update_server(server_id: int, updates: Dict[str, Any]) -> Optional[ServerModel]:
        with Session(engine) as session:
            server = session.get(ServerModel, server_id)
            if not server:
                return None
            for key, value in updates.items():
                if hasattr(server, key):
                    setattr(server, key, value)
            server.updated_at = datetime.utcnow()
            session.add(server)
            session.commit()
            session.refresh(server)
            _load_attrs(server)
            session.expunge(server)
            return server

    @staticmethod
    def delete_server(server_id: int) -> bool:
        with Session(engine) as session:
            server = session.get(ServerModel, server_id)
            if not server:
                return False
            session.delete(server)
            session.commit()
            return True

    @staticmethod
    def get_allocated_ports() -> Set[int]:
        with Session(engine) as session:
            stmt = select(ServerModel)
            servers = session.exec(stmt).all()
            ports = set()
            for s in servers:
                ports.add(s.game_port)
                ports.add(s.query_port)
                ports.add(s.rest_api_port)
            return ports


def _load_attrs(server: ServerModel):
    """Eagerly load all attributes before expunging from session."""
    _ = server.id
    _ = server.name
    _ = server.container_name
    _ = server.image_name
    _ = server.game_port
    _ = server.query_port
    _ = server.rest_api_port
    _ = server.data_volume_name
    _ = server.saves_path
    _ = server.mods_path
    _ = server.logicmods_path
    _ = server.nativemods_path
    _ = server.server_name
    _ = server.server_description
    _ = server.server_password
    _ = server.admin_password
    _ = server.max_players
    _ = server.env_vars
    _ = server.created_at
    _ = server.updated_at
    _ = server.workshop_dir
